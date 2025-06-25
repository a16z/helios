use crate::framework::BenchmarkResult;
use std::fmt::Write;

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        "0".to_string()
    } else if bytes < 1024 {
        format!("{}", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub struct BenchmarkReport {
    results: Vec<BenchmarkResult>,
}

impl BenchmarkReport {
    pub fn new(results: Vec<BenchmarkResult>) -> Self {
        Self { results }
    }

    pub fn print(&self) {
        println!("\n{}", self.generate());
    }

    pub fn generate(&self) -> String {
        let mut output = String::new();

        // Calculate column widths dynamically
        let mut name_width = "Benchmark".len();
        let mut time_width = "Time (ms)".len();
        let mut bytes_width = "Data Downloaded".len();
        let mut requests_width = "Requests".len();

        for result in &self.results {
            name_width = name_width.max(result.name.len() + 2);
            time_width = time_width.max(format!("{} ms", result.helios_time.as_millis()).len());
            time_width = time_width.max(format!("{} ms", result.rpc_time.as_millis()).len());
            time_width = time_width.max("99.99x ↑".len()); // For multiple + arrow

            let helios_bytes_str = format_bytes(result.helios_bytes);
            let rpc_bytes_str = format_bytes(result.rpc_bytes);
            bytes_width = bytes_width.max(helios_bytes_str.len() + 2);
            bytes_width = bytes_width.max(rpc_bytes_str.len() + 2);
            bytes_width = bytes_width.max("99.99x ↑".len());

            requests_width = requests_width.max(result.helios_requests.to_string().len());
            requests_width = requests_width.max(result.rpc_requests.to_string().len());
            requests_width = requests_width.max("99.99x ↑".len());
        }

        // Add some padding
        name_width += 4;
        time_width += 2;
        bytes_width += 2;
        requests_width += 2;

        writeln!(&mut output, "╔══════════════════════════════════════════════════════════════════════════════════════════════╗").unwrap();
        writeln!(&mut output, "║                              Helios vs Standard RPC Benchmark Report                         ║").unwrap();
        writeln!(&mut output, "╚══════════════════════════════════════════════════════════════════════════════════════════════╝").unwrap();
        writeln!(&mut output).unwrap();

        // Build dynamic table borders
        let top_border = format!(
            "┌{:─<name_width$}┬{:─<time_width$}┬{:─<bytes_width$}┬{:─<requests_width$}┬{:─<10}┐",
            "",
            "",
            "",
            "",
            "",
            name_width = name_width,
            time_width = time_width,
            bytes_width = bytes_width,
            requests_width = requests_width
        );

        let mid_border = format!(
            "├{:─<name_width$}┼{:─<time_width$}┼{:─<bytes_width$}┼{:─<requests_width$}┼{:─<10}┤",
            "",
            "",
            "",
            "",
            "",
            name_width = name_width,
            time_width = time_width,
            bytes_width = bytes_width,
            requests_width = requests_width
        );

        let bottom_border = format!(
            "└{:─<name_width$}┴{:─<time_width$}┴{:─<bytes_width$}┴{:─<requests_width$}┴{:─<10}┘",
            "",
            "",
            "",
            "",
            "",
            name_width = name_width,
            time_width = time_width,
            bytes_width = bytes_width,
            requests_width = requests_width
        );

        writeln!(&mut output, "{}", top_border).unwrap();
        writeln!(
            &mut output,
            "│{:^name_width$}│{:^time_width$}│{:^bytes_width$}│{:^requests_width$}│{:^10}│",
            "Benchmark",
            "Time (ms)",
            "Data Downloaded",
            "Requests",
            "Validated",
            name_width = name_width,
            time_width = time_width,
            bytes_width = bytes_width,
            requests_width = requests_width
        )
        .unwrap();
        writeln!(&mut output, "{}", mid_border).unwrap();

        for (idx, result) in self.results.iter().enumerate() {
            let time_multiple = result.time_diff_multiple();
            let bytes_multiple = result.bytes_diff_multiple();
            let requests_multiple = result.requests_diff_multiple();

            let validation_status = if result.validated { "✓" } else { "✗" };

            // Benchmark name row
            writeln!(
                &mut output,
                "│{:name_width$}│{:time_width$}│{:bytes_width$}│{:requests_width$}│{:^10}│",
                format!(" {}", result.name),
                "",
                "",
                "",
                "",
                name_width = name_width,
                time_width = time_width,
                bytes_width = bytes_width,
                requests_width = requests_width
            )
            .unwrap();

            // Helios row
            writeln!(
                &mut output,
                "│{:name_width$}│{:>time_width$}│{:>bytes_width$}│{:>requests_width$}│{:^10}│",
                "   Helios",
                format!("{} ms", result.helios_time.as_millis()),
                format!("{} B", format_bytes(result.helios_bytes)),
                result.helios_requests.to_string(),
                validation_status,
                name_width = name_width,
                time_width = time_width,
                bytes_width = bytes_width,
                requests_width = requests_width
            )
            .unwrap();

            // RPC row
            writeln!(
                &mut output,
                "│{:name_width$}│{:>time_width$}│{:>bytes_width$}│{:>requests_width$}│{:^10}│",
                "   RPC",
                format!("{} ms", result.rpc_time.as_millis()),
                format!("{} B", format_bytes(result.rpc_bytes)),
                result.rpc_requests.to_string(),
                "",
                name_width = name_width,
                time_width = time_width,
                bytes_width = bytes_width,
                requests_width = requests_width
            )
            .unwrap();

            let time_indicator = if (time_multiple - 1.0).abs() < 0.1 {
                "→"
            } else if time_multiple > 1.0 {
                "↑"
            } else {
                "↓"
            };

            let bytes_indicator = if bytes_multiple == 0.0 {
                "↓" // Helios uses 0 bytes
            } else if (bytes_multiple - 1.0).abs() < 0.1 {
                "→"
            } else if bytes_multiple > 1.0 {
                "↑"
            } else {
                "↓"
            };

            let requests_indicator = if requests_multiple == 0.0 {
                "↓" // Helios uses 0 requests
            } else if (requests_multiple - 1.0).abs() < 0.1 {
                "→"
            } else if requests_multiple > 1.0 {
                "↑"
            } else {
                "↓"
            };

            // Difference row
            let time_str = format!("{:.2}x {}", time_multiple, time_indicator);
            let bytes_str = if bytes_multiple == 0.0 {
                format!("0x {}", bytes_indicator)
            } else {
                format!("{:.2}x {}", bytes_multiple, bytes_indicator)
            };
            let requests_str = if requests_multiple == 0.0 {
                format!("0x {}", requests_indicator)
            } else {
                format!("{:.2}x {}", requests_multiple, requests_indicator)
            };

            writeln!(
                &mut output,
                "│{:name_width$}│{:>time_width$}│{:>bytes_width$}│{:>requests_width$}│{:^10}│",
                "   Difference",
                time_str,
                bytes_str,
                requests_str,
                "",
                name_width = name_width,
                time_width = time_width,
                bytes_width = bytes_width,
                requests_width = requests_width
            )
            .unwrap();

            // Row separator (not after last item)
            if idx < self.results.len() - 1 {
                writeln!(&mut output, "{}", mid_border).unwrap();
            }
        }

        // Bottom border
        writeln!(&mut output, "{}", bottom_border).unwrap();

        writeln!(&mut output).unwrap();
        writeln!(&mut output, "Legend:").unwrap();
        writeln!(&mut output, "  ↑ Higher than RPC (worse)").unwrap();
        writeln!(&mut output, "  ↓ Lower than RPC (better)").unwrap();
        writeln!(&mut output, "  → Similar to RPC (±10%)").unwrap();

        output
    }

    pub fn summary(&self) -> String {
        String::new()
    }
}
