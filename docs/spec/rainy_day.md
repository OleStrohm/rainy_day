# Rainy Day

## Setup

r[store.init]
The store MUST be initialized to only contain a single file called `rainy_day`.

## Encrypt

r[encrypt.contents]
Files MUST be encrypted on the client side

r[encrypt.path]
File path MUST be encrypted on the client side

## Decrypt

r[decrypt.contents]
Files MUST be decrypted on the client side

## Insertion

r[insert.file]
Files MUST be inserted using a hashed path, encrypted path, and contents

## Retrieval

r[retrieve.by.path]
File contents MUST be retrievable with the hashed path
