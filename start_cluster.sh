#!/bin/bash

# Start validator nodes
for i in {1..4}
do
   /tmp/node${i}/script/start.sh --node node${i}
done

# Start VFN nodes
for i in {1..2}
do
   /tmp/vfn${i}/script/start.sh --node vfn${i}
done