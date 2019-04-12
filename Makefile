LINUX_RELEASE := $(shell lsb_release -cs)

default: build
install-dep:
	curl https://sh.rustup.rs -sSf | sh
	sudo apt-get install libssl-dev pkg-config moreutils
	echo Install docker
	sudo apt-get update
	sudo apt-get install apt-transport-https ca-certificates curl gnupg-agent software-properties-common
	curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
	sudo -E add-apt-repository "deb [arch=amd64] https://download.docker.com/linux/ubuntu $(LINUX_RELEASE) stable"
	sudo apt-get update
	sudo apt-get install docker-ce docker-ce-cli containerd.io
	sudo usermod -a -G docker ubuntu
build:
	cargo build

run: build
	RUST_BACKTRACE=1 cargo run config.yaml
