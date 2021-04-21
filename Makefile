generate: central #zerotier

central:
	bash generate.sh central

zerotier:
	bash generate.sh zerotier

.PHONY: generate central zerotier
