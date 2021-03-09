SHELL := /bin/bash

.PHONY: documentation lint cargoclean build-pi spinup-rabbit spinup-dns cleanup reboot

documentation:
	cargo doc --open

lint:
	cargo clippy

cargoclean:
	cargo sweep -t 7

build-pi:
	cargo build --release --target armv7-unknown-linux-gnueabihf --features vendored-openssl
	echo "build result can be found at ./target/armv7-unknown-linux-gnueabihf/debug/Kraken"

spinup-rabbit:
	docker run -d --hostname rabbitmq.service.dev -p 5672:5672 -p 15672:15672 rabbitmq:3-management

# credit to https://stackoverflow.com/questions/37242217/access-docker-container-from-host-using-containers-name
spinup-dns:
	docker run --hostname dns.mageddo --restart=unless-stopped -p 5380:5380 -v /var/run/docker.sock:/var/run/docker.sock -v /etc/resolv.conf:/etc/resolv.conf defreitas/dns-proxy-server

cleanup:
	(docker ps -a | grep tcp) && docker stop $$(docker ps -aq)
	docker system prune -f

reboot:
	make cleanup
	cargo run

stop-env-commit:
	git update-index --assume-unchanged .env

start-env-commit:
	git update-index --no-assume-unchanged .env