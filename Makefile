SHELL := /bin/bash

.PHONY: documentation spinup-rabbit spinup-dns cleanup reboot

documentation:
	cargo doc --open

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