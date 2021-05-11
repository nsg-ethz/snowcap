import json
import os
import shutil
import pandas as pd
import numpy as np
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_10"
else:
    root_folder = "eval_sigcomm2021/result_10"

####################
# prepare the data #
####################

with open(os.path.join(root_folder, "raw_output")) as fp:
    # skip all lines until the results
    while fp.readline().strip() != "Results:":
        pass
    # read the four important lines
    result = [fp.readline(), fp.readline(), fp.readline(), fp.readline()]

# format the lines
result = {line.split(":")[0].strip(): line.split(":")[1].strip() for line in result}
result = {k: (int(v.split(" ")[0]), float(v.split(" ")[1].strip(' ()%')) / 100) for k, v in result.items()}
result = {k: f"{v[1] * 100:.1f}\% ({v[0]})" for k, v in result.items()}

true_positive = result['True Positive']
true_negative = result['True Negative']
false_positive = result['False Positive']
false_negative = result['False Negative']

###########################
# generate the latex plot #
###########################

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
  & Prediction: $\neg \phi$ & Prediction: $\phi$ \\
\midrule
\multirow{{2}}{{*}}{{Measurement: $\neg \phi$}} & true negative & false positive\\
& {true_negative} & {false_positive}\\
\midrule
\multirow{{2}}{{*}}{{Measurement: $\phi$}} & false negative & true positive \\
& {false_negative} & {true_positive} \\
\toprule
\end{{tabular}}
\end{{document}}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex table.tex")
plot_pdf_file = os.path.join(tikz_folder, "table.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "table_10.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
