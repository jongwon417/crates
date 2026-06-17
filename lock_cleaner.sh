#!/bin/bash

find . -mindepth 2 -type f -name "*.lock" -exec rm -v {} \;