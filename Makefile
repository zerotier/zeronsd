generate:
	rm -rf ./gen
	mkdir -p ./gen
	docker pull openapitools/openapi-generator-cli:latest
	docker run --rm -u $$(id -u):$$(id -g) -v ${PWD}/api-spec.json:/api-spec.json -v ${PWD}/gen:/swagger openapitools/openapi-generator-cli generate \
		-i /api-spec.json \
		-g rust \
		-o /swagger
	grep -v default-features gen/Cargo.toml > tmp && mv tmp gen/Cargo.toml
