# this fun little grep just extracts the version information from Cargo.toml.
CARGO_VERSION=$$(grep version Cargo.toml | head -1 | awk '{ print $$3 }' | sed 's/"//g')

build: test
	cargo build

test:
	cargo test --lib

test-integration:
ifneq (${SKIP},)
	TOKEN=$$(cat test-token.txt) sudo -E bash -c "$$(which cargo) test ${RUN_TEST} -- --skip '${SKIP}' --nocapture --test-threads 1"
else
	TOKEN=$$(cat test-token.txt) sudo -E bash -c "$$(which cargo) test ${RUN_TEST} -- --nocapture --test-threads 1"
endif

docker-image:
	docker build -t zerotier/zeronsd .

docker-image-package:
	docker build --build-arg IS_LOCAL=1 -t zerotier/zeronsd:$(CARGO_VERSION) .
	docker build -f Dockerfile.alpine -t zerotier/zeronsd:alpine-$(CARGO_VERSION) .

docker-image-push: docker-image-package
	docker push zerotier/zeronsd:$(CARGO_VERSION)
	docker push zerotier/zeronsd:alpine-$(CARGO_VERSION)
	docker tag zerotier/zeronsd:$(CARGO_VERSION) zerotier/zeronsd:latest
	docker tag zerotier/zeronsd:alpine-$(CARGO_VERSION) zerotier/zeronsd:alpine-latest
	docker push zerotier/zeronsd:latest
	docker push zerotier/zeronsd:alpine-latest

packagedir:
	mkdir -p target/packages

package-ubi: packagedir packages-out
	docker build -f Dockerfile.ubi -t zeronsd-packages-ubi .
	docker run -it -v ${PWD}:/code -w /code --rm zeronsd-packages-ubi bash -c ". /root/.cargo/env && cargo build --release && cargo generate-rpm && mv /code/target/generate-rpm/*.rpm /code/target/packages"

package-ubuntu22: packagedir packages-out
	docker build -f Dockerfile.ubuntu -t zeronsd-packages-ubuntu .
	docker run -it -v ${PWD}:/code -w /code --rm zeronsd-packages-ubuntu bash -c "cargo deb --variant ubuntu22 && mv /code/target/debian/*.deb /code/target/packages"

package-debian: packagedir packages-out
	docker build -f Dockerfile.packages -t zeronsd-packages .
	docker run -it -v ${PWD}:/code -w /code --rm zeronsd-packages bash -c ". /root/.cargo/env && cargo deb && mv /code/target/debian/*.deb /code/target/packages"

packages: docker-image-package package-ubi package-ubuntu22 package-debian
	make packages-out

packages-out:
	@echo
	@find target/packages -name '*.deb' -o -name '*.rpm'
	@echo docker image "zerotier/zeronsd:$(CARGO_VERSION)" was tagged
	@echo
	@echo "The files were written as root. Please ensure they fit your needed permissions manually."
	@echo

clean:
	@echo
	@echo Running sudo to clean your target directory
	@echo
	sudo rm -rf target
	cargo clean

test-packages: clean packages
	docker run -v ${PWD}:/code --rm -it redhat/ubi8 bash -c "rpm -ivh /code/target/packages/\*.rpm && zeronsd --version"
	docker run -v ${PWD}:/code --rm -it debian:latest bash -c "dpkg -i /code/target/packages/zeronsd_${CARGO_VERSION}_amd64.deb && zeronsd --version"
	docker run -v ${PWD}:/code --rm -it ubuntu:focal bash -c "apt update -qq && apt install libssl1.1 libc6 -y && dpkg -i /code/target/packages/zeronsd_${CARGO_VERSION}_amd64.deb && zeronsd --version"
	docker run -v ${PWD}:/code --rm -it ubuntu:jammy bash -c "dpkg -i /code/target/packages/zeronsd-ubuntu22_${CARGO_VERSION}_amd64.deb && zeronsd --version"
	[ "$$(docker run --rm zerotier/zeronsd:$(CARGO_VERSION) --version)" = "zeronsd $(CARGO_VERSION)" ]
	make packages-out

.PHONY: docker-image docker-image-package \
	packages packages-out test-packages \
	clean package-debian package-ubuntu22 \
	package-ubi packagedir
