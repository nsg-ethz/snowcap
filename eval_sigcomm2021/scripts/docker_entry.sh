#! /bin/sh

# parse the first argument
export PATH="$HOME/.cargo/bin:$PATH"
cd snowcap

# default speedup of 1
if [ -z "$SPEEDUP" ]; then
    SPEEDUP=1
fi

# Use the speedup factor
ITER_10000=$(echo "10000 / ${SPEEDUP}" | bc)
ITER_1000=$(echo "1000 / ${SPEEDUP}" | bc)
ITER_EXP_5=$(echo "1000 / ${SPEEDUP}" | bc)
ITER_EXP_9=$(echo "100 / ${SPEEDUP}" | bc)
ITER_EXP_10=$(echo "1000 / ${SPEEDUP}" | bc)

# limit the speedup factor that the minimal numbers are satisfied
if [ "${ITER_EXP_5}" -lt "500" ]; then
    ITER_EXP_5="500"
fi
if [ "${ITER_EXP_9}" -lt "10" ]; then
    ITER_EXP_9="10"
fi
if [ "${ITER_EXP_10}" -lt "100" ]; then
    ITER_EXP_10="100"
fi

# use number of threads
if [ -z "$THREADS" ]; then
    THREADS_PP=""
    THREADS_SM=""
else
    THREADS_PP="--num-threads ${THREADS}"
    THREADS_SM="--threads ${THREADS}"
fi

export RUST_LOG=none

# Experiment 1
mkdir eval_sigcomm2021/result_1
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "${topo}" != "PionierL3.gml" ]; then
        ./target/release/problem_probability ${THREADS_PP} -i ${ITER_10000} -n 10 -s FM2RR --many-prefixes eval_sigcomm2021/topology_zoo/${topo} probability -s -o eval_sigcomm2021/result_1/${topo}.json
    fi
done
python3.8 eval_sigcomm2021/scripts/plot_1.py

# Experiment 2
mkdir eval_sigcomm2021/result_2
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    ./target/release/problem_probability ${THREADS_PP} -i ${ITER_10000} -n 1 -s NetAcq --many-prefixes --seed 10 eval_sigcomm2021/topology_zoo/${topo} cost -a -f 100 -o eval_sigcomm2021/result_2/${topo}.json
done
python3.8 eval_sigcomm2021/scripts/plot_2.py

# Experiment 3
mkdir eval_sigcomm2021/result_3
for N in 1 2 3 4 5 6 7 8 9; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --random --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i ${ITER_1000} example chain-gadget -r ${N}
done
for N in 10 11 12 13 14 15 16 17 18 19 20 30 40 50 60 70 80 90 100; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i ${ITER_1000} example chain-gadget -r ${N}
done
python3.8 eval_sigcomm2021/scripts/plot_3.py

# Experiment 4
mkdir eval_sigcomm2021/result_4
for N in 1 2 3 4 5; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --random --tree --main --json eval_sigcomm2021/result_4/n${N}.json -i ${ITER_1000} example difficult-gadget-repeated -r ${N}
done
for N in 6 7 8 9 10 11 12 13 14 15 16; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --random --main --json eval_sigcomm2021/result_4/n${N}.json -i ${ITER_1000} example difficult-gadget-repeated -r ${N}
done
for N in 17 18 19 20; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --main --json eval_sigcomm2021/result_4/n${N}.json -i ${ITER_1000} example difficult-gadget-repeated -r ${N}
done
python3.8 eval_sigcomm2021/scripts/plot_4.py

# Experiment 5
mkdir eval_sigcomm2021/result_5
for r in 1 3 5 7 9 11 13; do
    for v in $(seq 0 66); do
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --main -i ${ITER_EXP_5} --json eval_sigcomm2021/result_5/r${r}_v${v}.json example variable-abilene-network -i ${v} -r ${r}
    done
done
python3.8 eval_sigcomm2021/scripts/plot_5.py

# Experiment 6
mkdir eval_sigcomm2021/result_6
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_6/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} IGPx2
    fi
done
python3.8 eval_sigcomm2021/scripts/plot_6-8.py 6

# Experiment 7
mkdir eval_sigcomm2021/result_7
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_7/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} LPx2
    fi
done
python3.8 eval_sigcomm2021/scripts/plot_6-8.py 7

# Experiment 8
mkdir eval_sigcomm2021/result_8
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_8/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} NetAcq
    fi
done
python3.8 eval_sigcomm2021/scripts/plot_6-8.py 8

# Experiment 9
mkdir eval_sigcomm2021/result_9
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --main -i ${ITER_EXP_9} -t 100000 --json eval_sigcomm2021/result_9/${topo}.strat.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
        ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main -i ${ITER_EXP_9} -t 100000 --json eval_sigcomm2021/result_9/${topo}.optim.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --random -i ${ITER_10000} --json eval_sigcomm2021/result_9/${topo}.rand.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
    fi
done
python3.8 eval_sigcomm2021/scripts/plot_9.py

# Experiment 10
mkdir eval_sigcomm2021/result_10
RUST_LOG=none ./target/release/snowcap_main transient ${THREADS_PP} eval_sigcomm2021/topology_zoo/SwitchL3.gml -i ${ITER_EXP_10} -r | tee eval_sigcomm2021/result_10/raw_output
python3.8 eval_sigcomm2021/scripts/table_10.py
