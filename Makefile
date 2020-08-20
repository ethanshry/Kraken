.PHONY: documentation build-rabbit run-rabbit

install:
	echo "TODO: Implement Installation Script"

documentation:
	cargo doc

build-rabbit:
	docker run -d --hostname my-rabbit --name rabi -p 5672:5672 rabbitmq:3

run-rabbit:
	docker start rab