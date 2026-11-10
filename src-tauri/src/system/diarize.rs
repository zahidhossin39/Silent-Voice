//! Tells the people on a meeting's "Others" track apart: sherpa-onnx speaker
//! diarization (pyannote segmentation + a voice-embedding model, clustered).
//! Offline only: it needs the whole track, so it runs after Stop.

use libloading::Symbol;
use std::ffi::{c_char, c_float, c_int, c_void, CString};
use std::path::Path;

#[repr(C)]
struct PyannoteConfig {
    model: *const c_char,
}
#[repr(C)]
struct SegmentationConfig {
    pyannote: PyannoteConfig,
    num_threads: c_int,
    debug: c_int,
    provider: *const c_char,
}
#[repr(C)]
struct EmbeddingConfig {
    model: *const c_char,
    num_threads: c_int,
    debug: c_int,
    provider: *const c_char,
}
#[repr(C)]
struct ClusteringConfig {
    num_clusters: c_int,
    threshold: c_float,
}
#[repr(C)]
struct DiarizationConfig {
    segmentation: SegmentationConfig,
    embedding: EmbeddingConfig,
    clustering: ClusteringConfig,
    min_duration_on: c_float,
    min_duration_off: c_float,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct RawSegment {
    start: c_float,
    end: c_float,
    speaker: c_int,
}

/// sherpa's own clustering is only used to cut the track into short
/// one-voice stretches: this low threshold over-splits on purpose.
const STRETCHES: f32 = 0.3;
/// Voices (cosine of CAM++ voiceprints) at least this alike are one person.
/// Tuned on two real calls against a reference (Gemini listening to the whole
/// track): 0.55-0.65 gave the right count on both (5 and 2 people, ~88% of the
/// speech labelled alike); sherpa's single threshold got one or the other wrong.
const SAME_VOICE: f32 = 0.6;
/// A "voice" with less speech than this (ms) is folded into the nearest one:
/// one short line is not a new person.
const MIN_VOICE_MS: f32 = 4_000.0;
/// Stretches shorter than this (ms) are too short for a voiceprint to be
/// trusted when forming voices; they're still labelled with the nearest one.
const MIN_STRETCH_MS: u64 = 1_000;

/// One stretch of speech by speaker `who` (0-based, in no particular order).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    pub start_ms: u64,
    pub end_ms: u64,
    pub who: u32,
}

/// Who spoke when in `samples` (16 kHz mono), sorted by start. `people` fixes
/// how many voices there are; None counts them.
pub fn turns(dir: &Path, samples: &[f32], people: Option<u32>) -> Result<Vec<Turn>, String> {
    let mut stretches = turns_with(dir, samples, -1, STRETCHES)?;
    let clip = |t: &Turn| &samples[(t.start_ms as usize * 16).min(samples.len())..(t.end_ms as usize * 16).min(samples.len())];
    let prints = voiceprints(dir, &stretches.iter().map(clip).collect::<Vec<_>>())?;
    let spans: Vec<(u64, u64)> = stretches.iter().map(|t| (t.start_ms, t.end_ms)).collect();
    for (t, who) in stretches.iter_mut().zip(voices(&spans, &prints, people)) {
        t.who = who;
    }
    Ok(stretches)
}

/// Which voice each stretch (`spans[i]`, voiceprint `prints[i]`) is. Voices
/// are found by merging the most alike stretches (average voiceprint, weighted
/// by length) until none are SAME_VOICE alike, or until `people` are left;
/// then voices with under MIN_VOICE_MS of speech fold into the nearest. A
/// stretch without a voiceprint takes the voice of the nearest one with.
pub fn voices(spans: &[(u64, u64)], prints: &[Option<Vec<f32>>], people: Option<u32>) -> Vec<u32> {
    let ms = |i: usize| (spans[i].1 - spans[i].0) as f32;
    let average = |members: &[usize]| {
        let mut v = vec![0.0f32; prints.iter().flatten().next().map_or(0, Vec::len)];
        for &i in members {
            if let Some(p) = &prints[i] {
                v.iter_mut().zip(p).for_each(|(a, b)| *a += b * ms(i));
            }
        }
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(f32::EPSILON);
        v.iter_mut().for_each(|x| *x /= n);
        v
    };
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let mut groups: Vec<Vec<usize>> = (0..spans.len()).filter(|&i| prints[i].is_some() && spans[i].1 - spans[i].0 >= MIN_STRETCH_MS).map(|i| vec![i]).collect();
    if groups.is_empty() {
        return vec![0; spans.len()];
    }
    loop {
        let centres: Vec<Vec<f32>> = groups.iter().map(|g| average(g)).collect();
        let mut best = (f32::MIN, 0, 0);
        for a in 0..groups.len() {
            for b in a + 1..groups.len() {
                let like = dot(&centres[a], &centres[b]);
                if like > best.0 {
                    best = (like, a, b);
                }
            }
        }
        let enough = match people {
            Some(n) => groups.len() <= n.max(1) as usize,
            None => best.0 < SAME_VOICE,
        };
        if groups.len() < 2 || enough {
            break;
        }
        let merged = groups.swap_remove(best.2);
        groups[best.1].extend(merged);
    }
    // A fixed count is taken as given; otherwise a voice needs enough speech.
    if people.is_none() {
        let big: Vec<Vec<usize>> = groups.iter().filter(|g| g.iter().map(|&i| ms(i)).sum::<f32>() >= MIN_VOICE_MS).cloned().collect();
        if !big.is_empty() {
            groups = big;
        }
    }
    let centres: Vec<Vec<f32>> = groups.iter().map(|g| average(g)).collect();
    let nearest = |p: &[f32]| (0..centres.len()).max_by(|&a, &b| dot(&centres[a], p).total_cmp(&dot(&centres[b], p))).unwrap_or(0) as u32;
    let who: Vec<Option<u32>> = prints.iter().map(|p| p.as_deref().map(nearest)).collect();
    let mid = |i: usize| (spans[i].0 + spans[i].1) / 2;
    (0..who.len())
        .map(|i| who[i].or_else(|| (0..who.len()).filter(|&j| who[j].is_some()).min_by_key(|&j| mid(j).abs_diff(mid(i))).and_then(|j| who[j])).unwrap_or(0))
        .collect()
}

/// The voice model file in the speakers folder.
pub const VOICE_MODEL: &str = "voice.onnx";

/// `clusters` -1 = let `threshold` decide.
fn turns_with(dir: &Path, samples: &[f32], clusters: c_int, threshold: f32) -> Result<Vec<Turn>, String> {
    let lib = super::sherpa::lib()?;
    let c = |p: &Path| CString::new(p.to_string_lossy().into_owned()).map_err(|e| e.to_string());
    let seg = c(&dir.join("segmentation.int8.onnx"))?;
    let emb = c(&dir.join(VOICE_MODEL))?;
    let cpu = CString::new("cpu").unwrap();
    let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)) as c_int;
    let cfg = DiarizationConfig {
        segmentation: SegmentationConfig { pyannote: PyannoteConfig { model: seg.as_ptr() }, num_threads: threads, debug: 0, provider: cpu.as_ptr() },
        embedding: EmbeddingConfig { model: emb.as_ptr(), num_threads: threads, debug: 0, provider: cpu.as_ptr() },
        clustering: ClusteringConfig { num_clusters: clusters, threshold },
        min_duration_on: 0.3,
        min_duration_off: 0.5,
    };
    unsafe {
        let create: Symbol<unsafe extern "C" fn(*const DiarizationConfig) -> *const c_void> =
            lib.get(b"SherpaOnnxCreateOfflineSpeakerDiarization\0").map_err(|e| e.to_string())?;
        let destroy: Symbol<unsafe extern "C" fn(*const c_void)> =
            lib.get(b"SherpaOnnxDestroyOfflineSpeakerDiarization\0").map_err(|e| e.to_string())?;
        let process: Symbol<unsafe extern "C" fn(*const c_void, *const c_float, c_int) -> *const c_void> =
            lib.get(b"SherpaOnnxOfflineSpeakerDiarizationProcess\0").map_err(|e| e.to_string())?;
        let count: Symbol<unsafe extern "C" fn(*const c_void) -> c_int> =
            lib.get(b"SherpaOnnxOfflineSpeakerDiarizationResultGetNumSegments\0").map_err(|e| e.to_string())?;
        let sorted: Symbol<unsafe extern "C" fn(*const c_void) -> *const RawSegment> =
            lib.get(b"SherpaOnnxOfflineSpeakerDiarizationResultSortByStartTime\0").map_err(|e| e.to_string())?;
        let free_segs: Symbol<unsafe extern "C" fn(*const RawSegment)> =
            lib.get(b"SherpaOnnxOfflineSpeakerDiarizationDestroySegment\0").map_err(|e| e.to_string())?;
        let free_result: Symbol<unsafe extern "C" fn(*const c_void)> =
            lib.get(b"SherpaOnnxOfflineSpeakerDiarizationDestroyResult\0").map_err(|e| e.to_string())?;

        let sd = create(&cfg);
        if sd.is_null() {
            return Err("could not load the speaker models".into());
        }
        let result = process(sd, samples.as_ptr(), samples.len() as c_int);
        let mut out = Vec::new();
        if !result.is_null() {
            let n = count(result).max(0) as usize;
            let segs = sorted(result);
            if !segs.is_null() {
                out = std::slice::from_raw_parts(segs, n)
                    .iter()
                    .map(|s| Turn { start_ms: (s.start * 1000.0) as u64, end_ms: (s.end * 1000.0) as u64, who: s.speaker.max(0) as u32 })
                    .collect();
                free_segs(segs);
            }
            free_result(result);
        }
        destroy(sd);
        if result.is_null() {
            return Err("speaker separation failed".into());
        }
        Ok(out)
    }
}

/// A voiceprint (unit length) for each of `clips` (16 kHz mono), from the same
/// voice model diarization uses; None for a clip too short to tell.
pub fn voiceprints(dir: &Path, clips: &[&[f32]]) -> Result<Vec<Option<Vec<f32>>>, String> {
    let lib = super::sherpa::lib()?;
    let emb = CString::new(dir.join(VOICE_MODEL).to_string_lossy().into_owned()).map_err(|e| e.to_string())?;
    let cpu = CString::new("cpu").unwrap();
    let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)) as c_int;
    let cfg = EmbeddingConfig { model: emb.as_ptr(), num_threads: threads, debug: 0, provider: cpu.as_ptr() };
    unsafe {
        let create: Symbol<unsafe extern "C" fn(*const EmbeddingConfig) -> *const c_void> =
            lib.get(b"SherpaOnnxCreateSpeakerEmbeddingExtractor\0").map_err(|e| e.to_string())?;
        let destroy: Symbol<unsafe extern "C" fn(*const c_void)> =
            lib.get(b"SherpaOnnxDestroySpeakerEmbeddingExtractor\0").map_err(|e| e.to_string())?;
        let dim: Symbol<unsafe extern "C" fn(*const c_void) -> c_int> =
            lib.get(b"SherpaOnnxSpeakerEmbeddingExtractorDim\0").map_err(|e| e.to_string())?;
        let new_stream: Symbol<unsafe extern "C" fn(*const c_void) -> *const c_void> =
            lib.get(b"SherpaOnnxSpeakerEmbeddingExtractorCreateStream\0").map_err(|e| e.to_string())?;
        let accept: Symbol<unsafe extern "C" fn(*const c_void, c_int, *const c_float, c_int)> =
            lib.get(b"SherpaOnnxOnlineStreamAcceptWaveform\0").map_err(|e| e.to_string())?;
        let finished: Symbol<unsafe extern "C" fn(*const c_void)> =
            lib.get(b"SherpaOnnxOnlineStreamInputFinished\0").map_err(|e| e.to_string())?;
        let ready: Symbol<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int> =
            lib.get(b"SherpaOnnxSpeakerEmbeddingExtractorIsReady\0").map_err(|e| e.to_string())?;
        let compute: Symbol<unsafe extern "C" fn(*const c_void, *const c_void) -> *const c_float> =
            lib.get(b"SherpaOnnxSpeakerEmbeddingExtractorComputeEmbedding\0").map_err(|e| e.to_string())?;
        let free_print: Symbol<unsafe extern "C" fn(*const c_float)> =
            lib.get(b"SherpaOnnxSpeakerEmbeddingExtractorDestroyEmbedding\0").map_err(|e| e.to_string())?;
        let free_stream: Symbol<unsafe extern "C" fn(*const c_void)> =
            lib.get(b"SherpaOnnxDestroyOnlineStream\0").map_err(|e| e.to_string())?;

        let ex = create(&cfg);
        if ex.is_null() {
            return Err("could not load the voice model".into());
        }
        let d = dim(ex).max(0) as usize;
        let prints = clips
            .iter()
            .map(|clip| {
                let s = new_stream(ex);
                if s.is_null() {
                    return None;
                }
                accept(s, 16_000, clip.as_ptr(), clip.len() as c_int);
                finished(s);
                let mut print = None;
                if ready(ex, s) != 0 {
                    let v = compute(ex, s);
                    if !v.is_null() {
                        let mut p = std::slice::from_raw_parts(v, d).to_vec();
                        free_print(v);
                        let norm = p.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if norm > 0.0 {
                            p.iter_mut().for_each(|x| *x /= norm);
                            print = Some(p);
                        }
                    }
                }
                free_stream(s);
                print
            })
            .collect();
        destroy(ex);
        Ok(prints)
    }
}

/// Voices at least this alike (cosine) are taken for the same person when a
/// line no turn covered is given a speaker.
const LIKE: f32 = 0.5;

/// Lines no turn covered (`who` None) get the speaker whose lines sound most
/// like them, when close enough. `prints[i]` is line i's voiceprint.
pub fn fill_in(who: &mut [Option<u32>], prints: &[Option<Vec<f32>>]) {
    // Each speaker's voice: the average of their lines' prints.
    let mut voices: Vec<(u32, Vec<f32>)> = Vec::new();
    for (w, p) in who.iter().zip(prints) {
        let (Some(w), Some(p)) = (w, p) else { continue };
        match voices.iter_mut().find(|(v, _)| v == w) {
            Some((_, sum)) => sum.iter_mut().zip(p).for_each(|(s, x)| *s += x),
            None => voices.push((*w, p.clone())),
        }
    }
    for (_, v) in &mut voices {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(f32::EPSILON);
        v.iter_mut().for_each(|x| *x /= norm);
    }
    for (w, p) in who.iter_mut().zip(prints) {
        let (None, Some(p)) = (&w, p) else { continue };
        let best = voices
            .iter()
            .map(|(v, voice)| (*v, voice.iter().zip(p).map(|(a, b)| a * b).sum::<f32>()))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        *w = best.filter(|(_, like)| *like >= LIKE).map(|(v, _)| v);
    }
}

/// Speech a speaker needs inside a line for the line to be split at them.
const MIN_PART_MS: u64 = 1_000;

/// Where `start_ms..end_ms` should be cut so each part is one speaker: empty
/// unless two or more speakers each say at least MIN_PART_MS in it. The cut
/// sits in the gap between them (or where the next one starts, if they overlap).
pub fn split_at_speakers(turns: &[Turn], start_ms: u64, end_ms: u64) -> Vec<(u64, u64)> {
    // Runs of one speaker, clipped to the line: (who, from, to, ms of speech).
    let mut runs: Vec<(u32, u64, u64, u64)> = Vec::new();
    for t in turns.iter().filter(|t| t.end_ms > start_ms && t.start_ms < end_ms) {
        let (a, b) = (t.start_ms.max(start_ms), t.end_ms.min(end_ms));
        match runs.last_mut() {
            Some(r) if r.0 == t.who => {
                r.2 = r.2.max(b);
                r.3 += b - a;
            }
            _ => runs.push((t.who, a, b, b - a)),
        }
    }
    // A speaker saying too little to be worth a line stays with their neighbour.
    runs.retain(|r| r.3 >= MIN_PART_MS);
    runs.dedup_by(|b, a| {
        let same = a.0 == b.0;
        if same {
            a.2 = b.2;
        }
        same
    });
    if runs.len() < 2 {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut from = start_ms;
    for w in runs.windows(2) {
        let cut = if w[0].2 <= w[1].1 { (w[0].2 + w[1].1) / 2 } else { w[1].1 };
        parts.push((from, cut));
        from = cut;
    }
    parts.push((from, end_ms));
    parts
}

/// The speaker heard most during `start_ms..end_ms`, numbered from 1 in order
/// of first appearance; None when no turn overlaps it.
pub fn who_spoke(turns: &[Turn], start_ms: u64, end_ms: u64) -> Option<u32> {
    let mut order: Vec<u32> = Vec::new();
    for t in turns {
        if !order.contains(&t.who) {
            order.push(t.who);
        }
    }
    let mut heard: Vec<(u32, u64)> = Vec::new();
    for t in turns {
        let overlap = t.end_ms.min(end_ms).saturating_sub(t.start_ms.max(start_ms));
        if overlap > 0 {
            match heard.iter_mut().find(|(w, _)| *w == t.who) {
                Some((_, ms)) => *ms += overlap,
                None => heard.push((t.who, overlap)),
            }
        }
    }
    let (who, _) = heard.into_iter().max_by_key(|(_, ms)| *ms)?;
    Some(order.iter().position(|w| *w == who)? as u32 + 1)
}


// Layout of every struct above, checked against the C header (sherpa_layout_tests.rs).
#[cfg(test)]
pub(crate) fn layout_lines() -> String {
    c_layout! {
        PyannoteConfig = SherpaOnnxOfflineSpeakerSegmentationPyannoteModelConfig { model }
        SegmentationConfig = SherpaOnnxOfflineSpeakerSegmentationModelConfig { pyannote, num_threads, debug, provider }
        EmbeddingConfig = SherpaOnnxSpeakerEmbeddingExtractorConfig { model, num_threads, debug, provider }
        ClusteringConfig = SherpaOnnxFastClusteringConfig { num_clusters, threshold }
        DiarizationConfig = SherpaOnnxOfflineSpeakerDiarizationConfig { segmentation, embedding, clustering, min_duration_on, min_duration_off }
        RawSegment = SherpaOnnxOfflineSpeakerDiarizationSegment { start, end, speaker }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start_ms: u64, end_ms: u64, who: u32) -> Turn {
        Turn { start_ms, end_ms, who }
    }

    #[test]
    fn a_line_goes_to_whoever_spoke_most_of_it_numbered_by_first_appearance() {
        let turns = [turn(0, 5_000, 7), turn(5_000, 6_000, 3), turn(6_000, 20_000, 3)];
        assert_eq!(who_spoke(&turns, 0, 4_000), Some(1));
        assert_eq!(who_spoke(&turns, 4_000, 12_000), Some(2));
        assert_eq!(who_spoke(&turns, 30_000, 40_000), None);
    }

    #[test]
    fn a_line_holding_two_speakers_is_cut_between_them() {
        // A asks (0-3 s), a 0.4 s gap, B answers (3.4-6 s).
        let turns = [turn(0, 3_000, 0), turn(3_400, 6_000, 1)];
        assert_eq!(split_at_speakers(&turns, 0, 6_000), vec![(0, 3_200), (3_200, 6_000)]);
        // Overlapping: cut where B starts.
        assert_eq!(split_at_speakers(&[turn(0, 3_000, 0), turn(2_500, 6_000, 1)], 0, 6_000), vec![(0, 2_500), (2_500, 6_000)]);
    }

    #[test]
    fn a_short_interjection_or_one_speaker_is_not_split() {
        // B's 0.5 s "yeah" is too little for its own line; A's two runs stay one.
        let turns = [turn(0, 3_000, 0), turn(3_000, 3_500, 1), turn(3_600, 6_000, 0)];
        assert!(split_at_speakers(&turns, 0, 6_000).is_empty());
        assert!(split_at_speakers(&[turn(0, 6_000, 0)], 0, 6_000).is_empty());
        // B talks mostly outside this line: only 0.5 s of it falls inside.
        assert!(split_at_speakers(&[turn(0, 4_000, 0), turn(3_500, 9_000, 1)], 0, 4_000).is_empty());
    }

    #[test]
    fn an_unlabelled_line_gets_the_voice_it_sounds_like_if_any() {
        let p = |v: [f32; 3]| {
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            Some(v.map(|x| x / n).to_vec())
        };
        let mut who = vec![Some(1), Some(2), Some(1), None, None, None];
        let prints = vec![p([1.0, 0.1, 0.0]), p([0.0, 1.0, 0.1]), p([0.9, 0.0, 0.1]), p([0.1, 0.9, 0.0]), p([0.0, 0.0, 1.0]), None];
        fill_in(&mut who, &prints);
        assert_eq!(who, vec![Some(1), Some(2), Some(1), Some(2), None, None], "like 2 / like nobody / too short");
    }

    /// A voiceprint near axis `axis` of 8, nudged by `wobble` so no two are equal.
    fn print_of(axis: usize, wobble: f32) -> Option<Vec<f32>> {
        let mut v: Vec<f32> = (0..8).map(|k| if k == axis { 1.0 } else { wobble * ((k * 7 + axis) % 5) as f32 / 5.0 }).collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        Some(v)
    }

    #[test]
    fn voices_are_counted_and_a_short_stray_voice_is_not_a_new_person() {
        // Three people taking 3 s turns in rotation, then a 2 s stray voice and
        // a stretch too short for a voiceprint.
        let mut spans = Vec::new();
        let mut prints = Vec::new();
        for k in 0..12u64 {
            spans.push((k * 3_000, k * 3_000 + 3_000));
            prints.push(print_of(k as usize % 3, 0.1 + 0.02 * k as f32));
        }
        spans.push((36_000, 38_000));
        prints.push(print_of(5, 0.1));
        spans.push((38_000, 38_300));
        prints.push(None);
        let who = voices(&spans, &prints, None);
        let mut people = who.clone();
        people.sort();
        people.dedup();
        assert_eq!(people.len(), 3, "{who:?}");
        assert!((0..12).all(|k| who[k] == who[k % 3]), "each person keeps one label: {who:?}");
        assert_eq!(who[13], who[12], "no voiceprint: the voice of the stretch next to it");
    }

    #[test]
    fn a_fixed_number_of_people_is_taken_as_given() {
        let spans: Vec<(u64, u64)> = (0..12u64).map(|k| (k * 3_000, k * 3_000 + 3_000)).collect();
        let prints: Vec<_> = (0..12).map(|k| print_of(k % 3, 0.1 + 0.02 * k as f32)).collect();
        let mut two = voices(&spans, &prints, Some(2));
        two.sort();
        two.dedup();
        assert_eq!(two.len(), 2);
    }

    #[test]
    #[ignore] // speaker models in SV_TEST_DIAR_DIR, SV_TEST_MEETINGS ';'-separated meeting paths, SV_TEST_COUNTS the people in each (';'-separated)
    fn real_calls_come_out_with_the_right_number_of_people() {
        let dir = std::env::var("SV_TEST_DIAR_DIR").map_or_else(|_| crate::models::registry::diarize_dir(), std::path::PathBuf::from);
        let counts = std::env::var("SV_TEST_COUNTS").unwrap();
        for (base, want) in std::env::var("SV_TEST_MEETINGS").unwrap().split(';').zip(counts.split(';')) {
            let samples: Vec<f32> = hound::WavReader::open(format!("{base}-others.wav")).unwrap().samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
            let t = std::time::Instant::now();
            let turns = turns(&dir, &samples, None).unwrap();
            let mut speech = std::collections::BTreeMap::new();
            turns.iter().for_each(|t| *speech.entry(t.who).or_insert(0) += t.end_ms - t.start_ms);
            println!("{base}: {} people in {:?}, ms each {speech:?}", speech.len(), t.elapsed());
            assert_eq!(speech.len(), want.parse::<usize>().unwrap(), "{base}");
        }
    }

    #[test]
    #[ignore] // speaker models + SV_TEST_MEETING (a meeting's path without ".json")
    fn unlabelled_lines_of_a_real_call_compared_to_each_voice() {
        let dir = std::env::var("SV_TEST_DIAR_DIR").map_or_else(|_| crate::models::registry::diarize_dir(), std::path::PathBuf::from);
        let base = std::env::var("SV_TEST_MEETING").unwrap();
        let t: crate::transcripts::Transcript = serde_json::from_str(&std::fs::read_to_string(format!("{base}.json")).unwrap()).unwrap();
        let samples: Vec<f32> = hound::WavReader::open(format!("{base}-others.wav")).unwrap().samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        let turns = turns(&dir, &samples, None).unwrap();
        let mut lines: Vec<crate::transcripts::Line> = t.lines.iter().filter(|l| l.speaker == Some(crate::transcripts::Speaker::Others)).cloned().collect();
        if std::env::var("SV_TEST_RECUT").is_ok() {
            // As a meeting cuts the track today (turns at pauses), not as it was saved.
            let mut c = crate::audio::longform::chunker(100).cut_at_pauses(32_000);
            let mut pieces = c.push(&samples);
            pieces.extend(c.finish());
            lines = pieces.iter().filter(|p| p.speech).map(|p| crate::transcripts::Line { start_ms: p.start as u64 / 16, end_ms: (p.start + p.samples.len()) as u64 / 16, text: String::new(), speaker: None, who: None }).collect();
        }
        let clip = |l: &crate::transcripts::Line| &samples[(l.start_ms as usize * 16).min(samples.len())..(l.end_ms as usize * 16).min(samples.len())];
        let prints = voiceprints(&dir, &lines.iter().map(clip).collect::<Vec<_>>()).unwrap();
        let who: Vec<Option<u32>> = lines.iter().map(|l| who_spoke(&turns, l.start_ms, l.end_ms)).collect();
        if let Ok(path) = std::env::var("SV_TEST_PRINTS_OUT") {
            // [start_ms, end_ms, who, print] per line, for analysis outside.
            let rows: Vec<_> = lines.iter().zip(&who).zip(&prints).map(|((l, w), p)| serde_json::json!([l.start_ms, l.end_ms, w, p])).collect();
            std::fs::write(path, serde_json::to_string(&rows).unwrap()).unwrap();
        }
        for (i, l) in lines.iter().enumerate() {
            let sims: Vec<String> = (0..lines.len())
                .filter(|&j| j != i && who[j].is_some())
                .filter_map(|j| Some(format!("{}:{:.2}", who[j]?, prints[i].as_ref()?.iter().zip(prints[j].as_ref()?).map(|(a, b)| a * b).sum::<f32>())))
                .collect();
            println!("{:>6} {:?} like [{}] {}", l.start_ms, who[i], sims.join(" "), l.text.chars().take(30).collect::<String>());
        }
        let mut filled = who.clone();
        fill_in(&mut filled, &prints);
        println!("before {who:?}\nafter  {filled:?}");
    }
}
