DOCKER_NAME ?= my-os
.PHONY: docker build_docker
	
docker:
	docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

build_docker:
	docker build -t ${DOCKER_NAME} .

run:
	cd kernel && make run
