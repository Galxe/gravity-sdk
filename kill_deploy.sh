ps -ef | grep gravity_consensus | awk '{print $2}' | xargs kill -9
