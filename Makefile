generate: central service

central:
	bash generate.sh central

service:
	bash generate.sh service

.PHONY: generate central service
