LINUX_RELEASE := $(shell lsb_release -cs)
SHELL := /bin/bash

default: build

install-docker:
	echo Install docker
	sudo apt-get update
	sudo apt-get install -y apt-transport-https ca-certificates curl gnupg-agent software-properties-common
	sudo mkdir /etc/docker
	# echo '{ "storage-driver": "overlay2" }' | sudo tee /etc/docker/daemon.json
	echo '{ "storage-driver": "overlay" }' | sudo tee /etc/docker/daemon.json
	curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
	sudo -E add-apt-repository "deb [arch=amd64] https://download.docker.com/linux/ubuntu $(LINUX_RELEASE) stable"
	sudo apt-get update
	sudo apt-get install -y docker-ce docker-ce-cli containerd.io
	sudo usermod -a -G docker ubuntu
	docker volume create CCACHE

install-nginx:
	sudo apt-get install -y nginx
	sudo chown -R `whoami` /var/www/html
	sudo sed -i -e 's/# First attempt to/autoindex on; #/g' /etc/nginx/sites-enabled/default
	rm -f /var/www/html/index.nginx-debian.html 
	mkdir /var/www/html/jobs
	sudo service nginx restart

install-pkg:
	sudo apt-get install -y make build-essential git
	sudo apt-get install -y libssl-dev pkg-config moreutils libpq-dev libsqlite3-dev

install-dep: install-pkg install-docker install-nginx
	echo Installed all dependencies
prepare-image:
	(cd docker; bash build)
regen-db: build
	rm db/s5ci.sqlite3 || true
	mkdir -p db
	go-s5ci/go-s5ci -c config.yaml rebuild-database -i

build:
	(cd go-s5ci; go get || true; go build)

