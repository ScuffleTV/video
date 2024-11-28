mkdir -p rsa ec256 ec384

openssl genrsa -out rsa/private.pem 2048
openssl rsa -in rsa/private.pem -outform PEM -pubout -out rsa/public.pem

openssl ecparam -outform PEM -name prime256v1 -genkey -noout -out ec256/private.key
openssl pkcs8 -topk8 -nocrypt -in ec256/private.key -out ec256/private.pem
openssl ec -in ec256/private.pem -pubout -out ec256/public.pem

rm ec256/private.key

openssl ecparam -outform PEM -name secp384r1 -genkey -noout -out ec384/private.key
openssl pkcs8 -topk8 -nocrypt -in ec384/private.key -out ec384/private.pem
openssl ec -in ec384/private.pem -pubout -out ec384/public.pem

rm ec384/private.key
