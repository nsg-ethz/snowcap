import json
import os
import shutil
import pandas as pd
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_1"
else:
    root_folder = "eval_sigcomm2021/result_1"

####################
# prepare the data #
####################

df = pd.DataFrame(columns=['random_permutations', 'random_router_order', 'insert_before_order'])
for filename in os.listdir(root_folder):
    if filename == "plot":
        continue
    filepath = os.path.join(root_folder, filename)
    with open(filepath, 'r') as fp:
        data = json.load(fp)
        for network in data:
            random_permut = 1.0 - network['random_permutations']['result']['success_rate']
            random_router = 1.0 - network['random_router_order']['result']['success_rate']
            insert_before = 1.0 - network['insert_before_order']['result']['success_rate']
            df = df.append({'random_permutations': random_permut,
                            'random_router_order': random_router,
                            'insert_before_order': insert_before},
                           ignore_index=True)
        print(f"{filename} loaded")

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
    area style,
    xlabel={Error Rate},
    ylabel={CDF},
    legend style={draw=none},
    xmin=0,
    ymin=0,
    xmax=1.05,
    ymax=1.05,
    xtick={0, 0.85, 1},
    xticklabels={0\%, 85\%, 100\%},
    ytick={0, 0.27, 0.5, 0.75, 0.89, 1},
    yticklabels={0\%, 27\%, 50\%, 75\%, 89\%, 100\%},
    x label style={at={(axis description cs:0.5, 0)}, anchor=south},
    axis y line=left,
    axis x line=bottom,
    height=4.5cm,
    width={0.9*\linewidth}
    ]

    \addplot+[hist={bins=500, data min=0, data max=1, cumulative=true, density=true, data filter/.code={1-x}}, draw=none, fill=cLightRed, fill opacity=0.3]
    table [y=insert_before_order, col sep=comma] {data.csv};

    \addplot+[hist={bins=500, data min=0, data max=1, cumulative=true, density=true, data filter/.code={1-x}}, draw=none, fill=cLightRed, fill opacity=0.5]
    table [y=random_router_order, col sep=comma] {data.csv};

    %\draw[dashed, thick, color=cRed] (axis cs:0, 0.5) -- (axis cs:1.05, 0.5);
    %\draw[dashed, thick, color=cRed] (axis cs:0, 0.89) -- (axis cs:1.05, 0.89);
    %\draw[dashed, thick, color=cRed] (axis cs:0.85, 0) -- (axis cs:0.85, 1.05);
    %\draw (axis cs:0.05, 0.625) node[right] {Insert-before-remove};
    %\draw (axis cs:0.05, 0.135) node[right] {Random order};
  \end{axis}
\end{tikzpicture}
\end{document}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "plot_1.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
