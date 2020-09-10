SHELL := /bin/bash

.PHONY: documentation build-rabbit run-rabbit cleanup

documentation:
	cargo doc --open

spinup-rabbit:
	docker run -d --hostname my-rabbit -p 5672:5672 -p 15672:15672 rabbitmq:3-management

cleanup:
	(docker ps -a | grep tcp) && docker stop $$(docker ps -aq)
	docker container prune -y
	docker images prune -y