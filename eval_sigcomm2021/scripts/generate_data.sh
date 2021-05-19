#! /bin/sh

# Perform all the checks

# if we are in the root folder and there exists a snowcap folder, then the
# docker container is running. In this case, simply change into the snowcap
# directory
if [ "$(pwd)" = "/" -a -d "snowcap" ]; then
    cd snowcap
fi

# make sure we are in the main directory. If not. go back and try again
while [ true ]; do
    if [ -d "eval_sigcomm2021" -a \
         -d "snowcap" -a \
         -d "snowcap_bencher" -a \
         -d "snowcap_ltl_parser" -a \
         -d "snowcap_main" -a \
         -d "snowcap_runtime" ]; then
        break
    fi
    cd ..
done

# check that cargo is installed, and the stable toolchain 1.49 or greater is used
which rustup > /dev/null
if [ $? -ne 0 ]; then
    echo "rustup is not found!"
    exit 1
fi

which cargo > /dev/null
if [ $? -ne 0 ]; then
    echo "cargo is not found!"
    exit 1
fi

rustup show active-toolchain | grep "^stable" > /dev/null
if [ $? -ne 0 ]; then
    echo "rustup active toolchain is not stable"
    exit 1
fi

rust_version_ok=$(echo "1.49 <= $(rustup show | grep '^rustc' | grep -o '[0-9]\+\.[0-9]\+')" | bc)
if [ $rust_version_ok -eq 0 ]; then
    echo "Version of rust must be at least 1.49".
    exit 1
fi

# check if the release binary is built
if [ -f "target/release/snowcap_main" -a -f "target/release/problem_probability" ]; then
    echo "Snowcap is compiled correctly!"
else
    # compile snowcap for the release channel
    cargo build --release
    if [ -f "target/release/snowcap_main" -a -f "target/release/problem_probability" ]; then
        echo "Snowcap could not be compiled!"
        exit 1
    fi
fi

# configure the logger to not show anything
export RUST_LOG=none

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

# Experiment 1
mkdir eval_sigcomm2021/result_1
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "${topo}" != "PionierL3.gml" ]; then
        ./target/release/problem_probability ${THREADS_PP} -i ${ITER_10000} -n 10 -s FM2RR --many-prefixes eval_sigcomm2021/topology_zoo/${topo} probability -s -o eval_sigcomm2021/result_1/${topo}.json
    fi
done

# Experiment 2
mkdir eval_sigcomm2021/result_2
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    ./target/release/problem_probability ${THREADS_PP} -i ${ITER_10000} -n 1 -s NetAcq --many-prefixes --seed 10 eval_sigcomm2021/topology_zoo/${topo} cost -a -f 100 -o eval_sigcomm2021/result_2/${topo}.json
done

# Experiment 3
mkdir eval_sigcomm2021/result_3
for N in 1 2 3 4 5 6 7 8 9; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --random --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i ${ITER_1000} example chain-gadget -r ${N}
done
for N in 10 11 12 13 14 15 16 17 18 19 20 30 40 50 60 70 80 90 100; do
    ./target/release/snowcap_main bench ${THREADS_SM} strategy --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i ${ITER_1000} example chain-gadget -r ${N}
done

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

# Experiment 5
mkdir eval_sigcomm2021/result_5
for r in 1 3 5 7 9 11 13; do
    for v in $(seq 0 66); do
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --main -i ${ITER_EXP_5} --json eval_sigcomm2021/result_5/r${r}_v${v}.json example variable-abilene-network -i ${v} -r ${r}
    done
done

# Experiment 6
mkdir eval_sigcomm2021/result_6
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_6/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} IGPx2
    fi
done

# Experiment 7
mkdir eval_sigcomm2021/result_7
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_7/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} LPx2
    fi
done

# Experiment 8
mkdir eval_sigcomm2021/result_8
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
       ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main --mif --mil --random -i ${ITER_10000} --json eval_sigcomm2021/result_8/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} NetAcq
    fi
done

# Experiment 9
mkdir eval_sigcomm2021/result_9
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" != "GtsCe.gml" ]; then
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --main -i ${ITER_EXP_9} -t 100000 --json eval_sigcomm2021/result_9/${topo}.strat.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
        ./target/release/snowcap_main bench ${THREADS_SM} optimizer --main -i ${ITER_EXP_9} -t 100000 --json eval_sigcomm2021/result_9/${topo}.optim.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
        ./target/release/snowcap_main bench ${THREADS_SM} strategy --random -i ${ITER_10000} --json eval_sigcomm2021/result_9/${topo}.rand.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
    fi
done

# Experiment 10
mkdir eval_sigcomm2021/result_10
RUST_LOG=none ./target/release/snowcap_main transient ${THREADS_PP} eval_sigcomm2021/topology_zoo/SwitchL3.gml -i ${ITER_EXP_10} -r | tee eval_sigcomm2021/result_10/raw_output
