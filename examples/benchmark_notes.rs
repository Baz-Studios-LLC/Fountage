//! Read-only synthetic analysis timing; does not measure UI frames or file I/O.
use fountage_core::writing::{self, Format};
use std::{hint::black_box, time::Instant};

fn main() {
    let mut measurements = Vec::new();
    for count in [1_000, 2_000, 4_000, 8_000] {
        let mut source = String::from("Title: Notes performance QA\r\n\r\n");
        for i in 0..count {
            source.push_str(&format!(
                "Zoë 🌙 [[Note {i}: café 日本語]] {{{{%m: beat}}}}\r\n"
            ));
        }
        let mut times = Vec::new();
        for _ in 0..3 {
            let start = Instant::now();
            let analysis = writing::analyze(black_box(&source), Format::Screenplay);
            times.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(analysis.notes.len(), count);
            assert_eq!(analysis.markers.len(), count);
            black_box(analysis);
        }
        times.sort_by(f64::total_cmp);
        measurements.push(serde_json::json!({"notes":count,"sourceBytes":source.len(),"medianMilliseconds":times[1]}));
    }
    println!("{}", serde_json::to_string_pretty(&measurements).unwrap());
}
