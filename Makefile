# this fun little grep just extracts the version information from Cargo.toml.
CARGO_VERSION=$$(grep version Cargo.toml | head -1 | awk '{ print $$3 }' | sed 's/"//g') .

build: test
	cargo build

test:
	cargo test

test-integration: test
	TOKEN=$$(cat test-token.txt) sudo -E bash -c "$$(which cargo) test -- --ignored --nocapture"

generate: central service

central:
	bash generate.sh central zerotier-central-api

service:
	bash generate.sh service zerotier-one-api

docker-image:
	docker build -t zerotier/zeronsd .

docker-image-package:
	docker build --build-arg IS_LOCAL=1 -t zerotier/zeronsd:$(CARGO_VERSION)

packages:
	make docker-image-package
	docker build -f Dockerfile.packages -t zeronsd-packages .
	docker run -it -v ${PWD}:/code -w /code --rm zeronsd-packages bash -c "cargo deb && cargo-generate-rpm"
	make packages-out

packages-out:
	@echo
	@find target -name '*.deb' -o -name '*.rpm'
	@echo docker image "zerotier/zeronsd:$(CARGO_VERSION)" was tagged
	@echo
	@echo "The files were written as root. Please ensure they fit your needed permissions manually."
	@echo

clean:
	@echo
	@echo Running sudo to clean your target directory
	@echo
	sudo rm -rf target zerotier-central-api/target zerotier-one-api/target 
	cargo clean

test-packages: clean
	make packages
	docker run -v ${PWD}:/code --rm -it centos rpm -ivh /code/target/generate-rpm/\*.rpm
	for image in debian ubuntu; do \
		docker run -v ${PWD}:/code --rm -it $$image \
			bash -c "apt update -qq && apt install libssl1.1 -y && dpkg -i /code/$$(find target -name '*.deb')"; \
	done
	docker run --rm zerotier/zeronsd:$(CARGO_VERSION) help 2>/dev/null
	make packages-out

.PHONY: generate central service \
	docker-image docker-image-package \
	packages packages-out test-packages \
	clean
