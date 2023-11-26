release:
	cargo publish
	cargo rpm build
	cargo deb

.PHONY: release
