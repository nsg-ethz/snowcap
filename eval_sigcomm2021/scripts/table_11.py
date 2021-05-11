import json
import os
import shutil
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_11"
else:
    root_folder = "eval_sigcomm2021/result_11"

####################
# prepare the data #
####################


def extract_data(filename):
    with open(filename, "r") as fp:
        data = json.load(fp)

    # get the total number of packets
    num_success = 0
    num_fail = 0
    for flow in data:
        for step in flow["paths"]:
            for path in step:
                if path["path"]:
                    num_success += path["count"]
                else:
                    num_fail += path["count"]

    time = (num_success + num_fail) / len(data) / 100

    return num_success, num_fail, time


rand_succ, rand_fail, rand_time = extract_data(os.path.join(root_folder, "random.json"))
snow_succ, snow_fail, snow_time = extract_data(os.path.join(root_folder, "snowcap.json"))

# generate the folder (delete if it already exists)
tikz_folder = root_folder

# generate the latex document
doc_file = os.path.join(tikz_folder, "table.tex")
doc = rf"""\documentclass{{standalone}}
\usepackage[english]{{babel}}
\usepackage{{booktabs}}
\usepackage{{multirow}}

\begin{{document}}
\begin{{tabular}}{{ccc}}
\toprule
  & \textbf{{Random approach}} & \textbf{{\textit{{Snowcap}}}} \\
  \midrule
transmitted & {100.0 * rand_succ / (rand_succ + rand_fail):.1f}\% ({rand_succ} packets) & {100.0 * snow_succ / (snow_succ + snow_fail):.1f}\% ({snow_succ} packets) \\
dropped & {100.0 * rand_fail / (rand_succ + rand_fail):.1f}\% ({rand_fail} packets) & {100.0 * snow_fail / (snow_succ + snow_fail):.1f}\% ({snow_fail} packets) \\
time & {rand_time:.1f}s & {snow_time:.1f}s \\
\toprule
\end{{tabular}}
\end{{document}}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex table.tex")
plot_pdf_file = os.path.join(tikz_folder, "table.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "table_11.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
