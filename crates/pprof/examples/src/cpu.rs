use std::io::Write;

use rand::Rng;

fn work() {
	let mut rnd = rand::thread_rng();

	let mut buf = vec![0u8; 1024 * 1024];
	rnd.fill(buf.as_mut_slice());

	loop {
		let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
		encoder.write_all(buf.as_slice()).unwrap();
		buf = encoder.finish().unwrap();
	}
}

fn main() {
	let cpu = scuffle_pprof::Cpu::new::<String>(1000, &[]);

	std::thread::spawn(work);

	let capture = cpu.capture(std::time::Duration::from_secs(10)).unwrap();

	std::fs::write("capture.pprof", capture).unwrap();
}
