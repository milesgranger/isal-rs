bench:
	cargo bench
	miniserve --index target/criterion/report/index.html -- target/criterion/
