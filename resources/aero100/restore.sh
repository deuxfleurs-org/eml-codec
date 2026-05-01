#!/usr/bin/env bash

wget https://git.deuxfleurs.fr/Deuxfleurs/aerogramme/media/branch/main/tests/emails/aero100.mbox.zstd -O - \
    | zstd -d - > aero100.mbox
