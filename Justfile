



# https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/8/html/securing_networks/creating-and-managing-tls-keys-and-certificates_securing-networks
certs:
	#!/usr/bin/env bash
	set -euo pipefail
	set -x

	cd certs
	mkdir -vp mainCA mainCA/newcerts mainCA/private
	touch mainCA/index.txt

	function genpkey() {
		# openssl genpkey -algorithm ec -pkeyopt ec_paramgen_curve:P-256 -out "${1:?}"
		# openssl genpkey -algorithm ed25519 -out "${1:?}"
		openssl genrsa -out "${1:?}" 2048
	}
	
	pushd mainCA
	# openssl genpkey -algorithm ec -pkeyopt ec_paramgen_curve:P-256 -out ./private/ca.key
	genpkey ./private/ca.key
	openssl req -key ./private/ca.key -new -x509 -days 3650 -config ../client_server.cnf -extensions ca-cert -out ca.crt
	# openssl req -key ./private/ca.key -new -x509 -days 3650 -addext \
	# 	keyUsage=critical,keyCertSign,cRLSign -subj "/CN=Gubernator/C=US" -out ca.crt
	chmod 600 ./private/ca.key
	# openssl genrsa -des3 -out ./private/cakey.pem 2048
	# openssl req -x509 -new -nodes -key ./private/cakey.pem -days 3650 -out cacert.pem

	popd
	genpkey ./guber.key
	openssl req -key ./guber.key -config client_server.cnf -extensions server-cert -new -out ./guber-cert.csr
	openssl x509 -req -in ./guber-cert.csr -CA mainCA/ca.crt -CAkey mainCA/private/ca.key -CAcreateserial -days 365 -extfile client_server.cnf -extensions server-cert -out ./guber-cert.crt


	# openssl genrsa -out ./client.key 2048
	# openssl req -new -out request.csr -key private.key -config openssl.cnf
	genpkey ./client.key
	openssl req -key ./client.key -config client_server.cnf -extensions client-cert -new -out ./client-cert.csr

	openssl x509 -req -in ./client-cert.csr -CA mainCA/ca.crt -CAkey mainCA/private/ca.key -CAcreateserial -days 365 -extfile client_server.cnf -extensions client-cert -out ./client-cert.crt


clear-certs:
	rm -rf certs/mainCA certs/*.key certs/*.crt certs/*.csr

server:
	cargo run --bin gub-server -- -vvvv -a 127.0.0.1:1832 -c certs/guber-cert.crt -k certs/guber.key

client:
	cargo run --bin gub-client -- -vvvv -a 127.0.0.1:1832 -d 127.0.0.1 -c certs/mainCA/ca.crt -k certs/client.key -p certs/client-cert.crt

test-openssl:
	openssl s_client -CAfile certs/mainCA/ca.crt -connect 127.0.0.1:1832

print-certs:
	openssl x509 -text -noout -in certs/mainCA/ca.crt
	openssl x509 -text -noout -in certs/client-cert.crt
	openssl x509 -text -noout -in certs/guber-cert.crt
