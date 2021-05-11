import json
import os
import shutil
import pandas as pd
import numpy as np
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_3"
else:
    root_folder = "eval_sigcomm2021/result_3"

####################
# prepare the data #
####################

df = pd.DataFrame(columns=['x', 'random', 'tree', 'snowcap'])
for filename in os.listdir(root_folder):
    if not filename.endswith("json"):
        continue
    filepath = os.path.join(root_folder, filename)
    with open(filepath, 'r') as fp:
        data = json.load(fp)
        x = data["num_commands"]
        tree_median = np.median(np.array([x["time"] for x in data["tree_result"]]))
        snowcap_median = np.median(np.array([x["time"] for x in data["strategy_result"]]))
        if data["random_result"]:
            random_median = np.median(np.array([x["time"] for x in data["random_result"]]))
        else:
            random_median = np.nan

        df = df.append({'x': x,
                        'random': random_median,
                        'tree': tree_median,
                        'snowcap': snowcap_median},
                        ignore_index=True)
df = df.sort_values('x', ignore_index=True)

###########################
# generate the latex plot #
###########################

# generate the folder (delete if it already exists)
tikz_folder = os.path.join(root_folder, "plot")
if os.path.exists(tikz_folder):
    shutil.rmtree(tikz_folder)
os.mkdir(tikz_folder)

# store the csv there
csv_file = os.path.join(tikz_folder, "data.csv")
df.to_csv(csv_file, sep=',', index=False)

# generate the latex document
doc_file = os.path.join(tikz_folder, "plot.tex")
doc = r"""\documentclass{standalone}
\usepackage[english]{babel}

% tikz
\usepackage{tikz}
\usepackage{pgfplots}
\usetikzlibrary{calc,shapes,arrows,decorations.markings}
\usepgfplotslibrary{fillbetween, groupplots, statistics}

\pgfplotsset{
  /pgfplots/evaluation line legend/.style 2 args={
    legend image code/.code={
      \draw[#2] (0mm, 1.5mm) rectangle (5mm, -1.5mm);
      \draw[#1] (0mm, 0mm) -- (5mm, 0mm);
    }
  }
}
\pgfplotsset{
  /pgfplots/evaluation dashed line legend/.style 2 args={
    legend image code/.code={
      \draw[#2] (0mm, 1.5mm) rectangle (5mm, -1.5mm);
      \draw[#1] (0mm, 0mm) -- (5mm, 0mm);
      \draw[thick, cBlue, dashed] (0mm, 0mm) -- (5mm, 0mm);
    }
  }
}

% colors
\usepackage{xcolor}
\definecolor{cLightRed}{HTML}{E74C3C}
\definecolor{cRed}{HTML}{C0392B}
\definecolor{cBlue}{HTML}{2980B9}
\definecolor{cLightBlue}{HTML}{3498DB}
\definecolor{cDarkBlue}{HTML}{10334A}
\definecolor{cGreen}{HTML}{27AE60}
\definecolor{cLightGreen}{HTML}{2ECC71}
\definecolor{cViolet}{HTML}{8E44AD}
\definecolor{cLightViolet}{HTML}{9B59B6}
\definecolor{cOrange}{HTML}{D35400}
\definecolor{cLightOrange}{HTML}{E67E22}
\definecolor{cYellow}{HTML}{F39C12}
\definecolor{cLightYellow}{HTML}{F1C40F}

\begin{document}
\begin{tikzpicture}
  \begin{axis}[
    height=5cm,
    width=8cm,
    axis x line = bottom,
    axis y line = left,
    ymax = 2.5,
    xmin = 2,
    xmax = 105,
    xlabel = {Number of commands},
    ylabel = {Time [s]},
    y label style={at={(axis description cs:0.2, 1.1)}, rotate=-90},
    legend cell align=left,
    legend style={
      draw=none,
      font=\small,
      anchor=north east,
      at={(1.01, 1.01)},
    },
    ]

    \addlegendimage{evaluation line legend={cRed, thick}{fill=none, draw=none}}
    \addlegendentry{Random permutations}

    \addlegendimage{evaluation dashed line legend={cGreen, thick}{fill=none, draw=none}}
    \addlegendentry{Snowcap $=$ Snowcap$^-$}

    \addplot+[mark=none, thick, cRed] table[x=x, y=random, col sep=comma] {data.csv};
    \addplot+[mark=none, thick, cGreen] table[x=x, y=tree, col sep=comma] {data.csv};
    \addplot+[mark=none, thick, cBlue, dashed] table[x=x, y=snowcap, col sep=comma] {data.csv};

  \end{axis}
\end{tikzpicture}
\end{document}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "plot_3.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
