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

/// Voices closer than this are one person. Measured on two real Bangla/English
/// calls with titanet-small: 0.5 split two people into 13-16, 0.8 into 7,
/// 1.2 merged them into 1; 1.0 matched who asked and who answered.
const SAME_VOICE: f32 = 1.0;

/// One stretch of speech by speaker `who` (0-based, in no particular order).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    pub start_ms: u64,
    pub end_ms: u64,
    pub who: u32,
}

/// Who spoke when in `samples` (16 kHz mono), sorted by start. `people` fixes
/// how many voices there are; None lets the SAME_VOICE threshold decide.
pub fn turns(dir: &Path, samples: &[f32], people: Option<u32>) -> Result<Vec<Turn>, String> {
    let lib = super::sherpa::lib()?;
    let c = |p: &Path| CString::new(p.to_string_lossy().into_owned()).map_err(|e| e.to_string());
    let seg = c(&dir.join("segmentation.int8.onnx"))?;
    let emb = c(&dir.join("embedding.onnx"))?;
    let cpu = CString::new("cpu").unwrap();
    let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)) as c_int;
    let cfg = DiarizationConfig {
        segmentation: SegmentationConfig { pyannote: PyannoteConfig { model: seg.as_ptr() }, num_threads: threads, debug: 0, provider: cpu.as_ptr() },
        embedding: EmbeddingConfig { model: emb.as_ptr(), num_threads: threads, debug: 0, provider: cpu.as_ptr() },
        clustering: ClusteringConfig { num_clusters: people.map_or(-1, |n| n as c_int), threshold: SAME_VOICE },
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
    #[ignore] // needs the speaker models in SV_TEST_DIAR_DIR, a meeting's -others.wav in SV_TEST_WAV; SV_TEST_PEOPLE fixes the count
    fn a_real_call_with_two_people_comes_out_as_two_speakers() {
        let dir = std::env::var("SV_TEST_DIAR_DIR").unwrap();
        let mut r = hound::WavReader::open(std::env::var("SV_TEST_WAV").unwrap()).unwrap();
        let samples: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        let t = std::time::Instant::now();
        let fixed = std::env::var("SV_TEST_PEOPLE").ok().map(|n| n.parse::<u32>().unwrap());
        let turns = turns(Path::new(&dir), &samples, fixed).unwrap();
        println!("{} turns in {:?}: {:?}", turns.len(), t.elapsed(), turns);
        let mut people: Vec<u32> = turns.iter().map(|t| t.who).collect();
        people.sort();
        people.dedup();
        match fixed {
            // A count above the real number of voices can't invent one.
            Some(n) => assert!(people.len() <= n as usize, "{} voices, asked for {n}", people.len()),
            None => assert_eq!(people.len(), 2),
        }
    }
}
