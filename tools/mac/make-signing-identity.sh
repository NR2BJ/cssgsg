#!/usr/bin/env bash
# 릴리스 서명용 자체 서명 인증서("cssgsg Code Signing")를 로그인 키체인에 만든다. 이미 있으면 아무것도 안 한다.
#
# 왜: ad-hoc 서명이면 macOS(TCC)가 앱을 바이너리 해시로 기억해서, 업데이트할 때마다 손쉬운 사용 권한이 풀린다.
#     같은 인증서로 서명하면 "이 인증서로 서명한 com.cssgsg.inputmethod.app"으로 기억해서 권한이 남는다.
# - 인증서를 "신뢰"로 설정하지는 않는다. 서명에는 필요 없다(Gatekeeper가 믿는 서명이 아니라 앱을 알아보는 용도).
# - 유효기간 20년. 키체인을 초기화해서 인증서가 없어지면 이 스크립트를 다시 돌리고 권한을 한 번 다시 켜면 된다.
# - 개인키는 키체인에만 둔다. 만들 때 쓴 임시 파일은 끝나면 지운다.
set -euo pipefail
NAME="cssgsg Code Signing"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -p codesigning "$KEYCHAIN" 2>/dev/null | grep -qF "\"$NAME\""; then
  echo "이미 있다: $NAME"
  security find-identity -p codesigning "$KEYCHAIN" | grep -F "\"$NAME\""
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
CNF
PASS="$(openssl rand -hex 16)"
openssl req -x509 -newkey rsa:2048 -sha256 -days 7300 -nodes \
  -keyout "$TMP/key.pem" -out "$TMP/cert.pem" -config "$TMP/cert.cnf" 2>/dev/null
# macOS security는 옛 PKCS12 알고리즘만 읽는다(기본값이면 "MAC verification failed").
openssl pkcs12 -export -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -name "$NAME" \
  -macalg sha1 -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES \
  -out "$TMP/identity.p12" -passout "pass:$PASS"
# -T: codesign은 키체인에 물어보지 않고 이 키를 쓸 수 있다.
security import "$TMP/identity.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign >/dev/null
echo "만들었다:"
security find-identity -p codesigning "$KEYCHAIN" | grep -F "\"$NAME\""
