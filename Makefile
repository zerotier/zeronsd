generate:
	rm -rf ./central
	mkdir -p ./central
	docker pull openapitools/openapi-generator-cli:latest
	docker run --rm -u $$(id -u):$$(id -g) -v ${PWD}/api-spec.json:/api-spec.json -v ${PWD}/central:/swagger openapitools/openapi-generator-cli generate \
		--package-name central \
		-i /api-spec.json \
		-g rust \
		-o /swagger
	grep -v default-features central/Cargo.toml > tmp && mv tmp central/Cargo.toml
