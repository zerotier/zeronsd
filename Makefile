generate: central service

central:
	bash generate.sh central zerotier-central-api

service:
	bash generate.sh service zerotier-one-api

.PHONY: generate central service
