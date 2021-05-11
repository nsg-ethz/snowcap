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

# check the python version
python -c 'import sys; exit(1) if sys.version_info.major < 3 or sys.version_info.minor < 8 else exit(0)'
if [ $? = 0 ]; then
    python_bin="python"
else
    python3 -c 'import sys; exit(1) if sys.version_info.major < 3 or sys.version_info.minor < 8 else exit(0)'
    if [ $? = 0 ]; then
        python_bin="python3"
    else
    	python3.8 -c 'import sys; exit(1) if sys.version_info.major < 3 or sys.version_info.minor < 8 else exit(0)'
    	if [ $? = 0 ]; then
            python_bin="python3.8"
    	else
       	    echo "Could not find a valid python binary! with version 3.8 or higher!"
            exit 1
    	fi
    fi
fi

echo "Using python binary: $(which ${python_bin})"

# check if numpy, pandas and matplotlib are installed
eval "${python_bin} -c 'import numpy'"
if [ $? != 0 ]; then
    echo "Numpy was not found!"
    exit 1
fi

eval "${python_bin} -c 'import pandas'"
if [ $? != 0 ]; then
    echo "Pandas was not found!"
    exit 1
fi

eval "${python_bin} -c 'from matplotlib import cbook'"
if [ $? != 0 ]; then
    echo "Matplotlib was not found, or matplotlib.cbook is not available!"
    exit 1
fi

# check that latex is available
which pdflatex > /dev/null
if [ $? != 0 ]; then
    echo "Pdflatex is not found in the PATH!"
    exit 1
fi

# check if we use precomputed data
if [ "${PRECOMPUTED_DATA}" = "yes" ]; then
    pre=" -pre"
else
    pre=""
fi

# generate all plots
eval "${python_bin} eval_sigcomm2021/scripts/plot_1.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_2.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_3.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_4.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_5.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_6-8.py 6${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_6-8.py 7${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_6-8.py 8${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/plot_9.py${pre}"
eval "${python_bin} eval_sigcomm2021/scripts/table_10.py${pre}"
if [ -d "eval_sigcomm2021/result_11" ]; then
    eval "${python_bin} eval_sigcomm2021/scripts/table_11.py${pre}"
fi
