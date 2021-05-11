import json
import os
import shutil
import pandas as pd
import numpy as np
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_5"
else:
    root_folder = "eval_sigcomm2021/result_5"

####################
# prepare the data #
####################

# read the data
result = {"complexity": list(range(0, 67))}
for r in [1, 3, 5, 7, 9, 11, 13]:
    seq = []
    num_commands = None
    for v in range(0, 67):
        filename = f"r{r}_v{v}.json"
        filepath = os.path.join(root_folder, filename)
        with open(filepath, 'r') as fp:
            data = json.load(fp)

        num_commands = data["num_commands"]
        seq.append(np.mean(np.array([x["time"] for x in data["strategy_result"]])))
    result[f"c{num_commands}"] = seq

# generate the dataframe
df = pd.DataFrame(result)
df.set_index('complexity', inplace=True)

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
df.to_csv(csv_file, sep=',')

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
    width=5cm,
    ymode=log,
    axis x line=bottom,
    axis y line=left,
    xlabel={Complexity of $\phi$},
    ylabel={Time [s]},
    y label style={at={(axis description cs:0.3, 1.1)}, rotate=-90},
    ymin = 0.0005,
    ymax = 80,
    xmax = 69,
    xtick = {0, 8, 39, 48, 66},
    legend cell align=left,
    legend style={
      draw=none,
      font=\small,
      anchor=west,
      at={(1, 0.5)},
    },
    ]

    \addlegendimage{evaluation line legend={cDarkBlue!100!cGreen, thick}{fill=none, draw=none}}
    \addlegendentry{$c=5$};
    \addlegendimage{evaluation line legend={cDarkBlue!066!cGreen, thick}{fill=none, draw=none}}
    \addlegendentry{$c=9$};
    \addlegendimage{evaluation line legend={cDarkBlue!033!cGreen, thick}{fill=none, draw=none}}
    \addlegendentry{$c=13$};
    \addlegendimage{evaluation line legend={cDarkBlue!000!cGreen, thick}{fill=none, draw=none}}
    \addlegendentry{$c=17$};
    \addlegendimage{evaluation line legend={cYellow, thick}{fill=none, draw=none}}
    \addlegendentry{$c=21$};
    \addlegendimage{evaluation line legend={cRed, thick}{fill=none, draw=none}}
    \addlegendentry{$c=25$};
    \addlegendimage{evaluation line legend={cViolet, thick}{fill=none, draw=none}}
    \addlegendentry{$c=29$};
    \addplot[thick, color=cDarkBlue!100!cGreen] table[x=complexity, y=c5, col sep=comma]{data.csv};
    \addplot[thick, color=cDarkBlue!066!cGreen] table[x=complexity, y=c9, col sep=comma]{data.csv};
    \addplot[thick, color=cDarkBlue!033!cGreen] table[x=complexity, y=c13, col sep=comma]{data.csv};
    \addplot[thick, color=cDarkBlue!000!cGreen] table[x=complexity, y=c17, col sep=comma]{data.csv};
    \addplot[thick, color=cYellow] table[x=complexity, y=c21, col sep=comma]{data.csv};
    \addplot[thick, color=cRed] table[x=complexity, y=c25, col sep=comma]{data.csv};
    \addplot[thick, color=cViolet] table[x=complexity, y=c29, col sep=comma]{data.csv};

  \end{axis}
\end{tikzpicture}
\end{document}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "plot_5.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
