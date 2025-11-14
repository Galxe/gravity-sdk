#!/bin/bash

# Stop validator nodes
for i in {1..4}
do
   /tmp/node${i}/script/stop.sh
done

# Stop VFN nodes
for i in {1..2}
do
   /tmp/vfn${i}/script/stop.sh
done

rm -rf /tmp/node*
rm -rf /tmp/vfn*