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

generate: central service

central:
	bash generate.sh central zerotier-central-api

service:
	bash generate.sh service zerotier-one-api

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
	sudo rm -rf target
	cargo clean

test-packages: clean
	make packages
	docker run -v ${PWD}:/code --rm -it centos rpm -ivh /code/target/generate-rpm/\*.rpm
	for image in debian ubuntu; do \
		docker run -v ${PWD}:/code --rm -it $$image \
			bash -c "apt update -qq && apt install libssl1.1 -y && dpkg -i /code/$$(find target -name '*.deb')"; \
	done
	[ "$$(docker run --rm zerotier/zeronsd:$(CARGO_VERSION) --version)" = "zeronsd $(CARGO_VERSION)" ]
	make packages-out

.PHONY: generate central service \
	docker-image docker-image-package \
	packages packages-out test-packages \
	clean
