LINUX_RELEASE := $(shell lsb_release -cs)

default: build
install-rust:
	sudo apt-get install -y make build-essential git
	curl https://sh.rustup.rs -sSf | sh -s -- -y
	sudo apt-get install -y libssl-dev pkg-config moreutils
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
build:
	cargo build

run: build
	RUST_BACKTRACE=1 cargo run -- --config config.yaml
