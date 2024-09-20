#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
WORKSPACE=$SCRIPT_DIR/..

cat ${WORKSPACE}/script/node.pid | xargs kill -9
rm ${WORKSPACE}/script/node.pid
