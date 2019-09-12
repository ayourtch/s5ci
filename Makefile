LINUX_RELEASE := $(shell lsb_release -cs)
SHELL := /bin/bash

default: build
install-rust:
	sudo apt-get install -y make build-essential git
	curl https://sh.rustup.rs -sSf | sh -s -- -y
	sudo apt-get install -y libssl-dev pkg-config moreutils libpq-dev libsqlite3-dev
	source ~/.cargo/env && cargo install diesel_cli --no-default-features --features postgres,sqlite
	mkdir db
	source ~/.cargo/env && diesel setup --database-url db/s5ci.sqlite3
	echo To finish Rust installation, please logout and login back

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
install-dep: install-rust install-docker install-nginx
	echo Installed all dependencies
prepare-image:
	(cd docker; bash build)
regen-db:
	diesel migration redo --database-url db/s5ci.sqlite3
	rustfmt src/schema.rs
	./dev-scripts/print-model >src/models.rs
rustfmt:
	find src -name '*.rs' -exec rustfmt {} \;

build:
	cargo build
	(cd go-s5ci; go get || true; go build)

run: build
	RUST_BACKTRACE=1 cargo run -- --config config.yaml
