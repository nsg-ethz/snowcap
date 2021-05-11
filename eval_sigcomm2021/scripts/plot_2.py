import json
import os
import shutil
from matplotlib import cbook
import pandas as pd
import numpy as np
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_2"
else:
    root_folder = "eval_sigcomm2021/result_2"


# Function to extract the boxplot data
def get_boxplot_data(data):
    assert(not np.any(np.isnan(data)))
    stats = cbook.boxplot_stats(data)[0]
    return (stats["med"], # median
            [stats["q1"], stats["q3"]], # Box (Quartals)
            [stats["whislo"], stats["whishi"]], # Whiskers
            stats["fliers"]) # Fliers


topos = list({x.split(".")[0] for x in os.listdir(root_folder) if ".gml.json" in x})
topos.sort()
# topos = ["Chinanet", "Harnet", "Iij", "Renater2010", "Uninett2011"]

# generate the folder (delete if it already exists)
tikz_folder = os.path.join(root_folder, "plot")
if os.path.exists(tikz_folder):
    shutil.rmtree(tikz_folder)
os.mkdir(tikz_folder)

####################
# prepare the data #
####################

result = []
idx = 0
for topo in reversed(topos):
    name = "".join([c for c in topo if not c.isdigit()])
    y_pos = idx * 1.5

    # read the data
    with open(os.path.join(root_folder, f"{topo}.gml.json"), 'r') as fp:
        data = json.load(fp)

    # extract the relevant data
    data = data[0]
    cost = np.array(data["random_permutations"]["cost"]["values"])
    ideal = data["ideal_cost"]

    # ignore all samples that have the same worst cost as the idel cost
    # if np.abs(ideal - cost.max()) < 1e-7:
        # continue

    # compute the statistics
    (median, box, whiskers, fliers) = get_boxplot_data(cost)

    # create the fliers file
    fliers_file = f"{topo}_fliers.csv"
    np.savetxt(os.path.join(tikz_folder, fliers_file), [fliers], delimiter='\n')

    # create the tikz plot
    plot = "    \\addplot+ [boxplot prepared = {\n" +\
        f"      draw position = {y_pos:.1f},\n" +\
        f"      upper whisker = {whiskers[1]},\n" +\
        f"      upper quartile = {box[1]},\n" +\
        f"      median = {median},\n" +\
        f"      lower quartile = {box[0]},\n" +\
        f"      lower whisker = {whiskers[0]},\n" +\
        f"    }}, draw=cRed, fill=cRed!20, mark=x, solid] table [y index=0] {{{fliers_file}}};\n" +\
         "    \\addplot+ [black!20, mark=none, solid, line width=0.2cm] coordinates\n" +\
        f"      {{(0, {y_pos:.1f}) ({ideal:.2f}, {y_pos:.1f})}};\n" +\
         "    \\addplot+ [black, mark=none, solid] coordinates\n" +\
        f"      {{({ideal:.2f}, {(y_pos - 0.5):.1f}) ({ideal:.2f}, {(y_pos + 0.5):.1f})}};\n"

    # store the result
    result.append({"name": name, "y_pos": y_pos, "plot": plot})

    idx += 1

###################################
# Start generating the Latex Plot #
###################################

# prepare all the ticks
ytick = ", ".join([str(x["y_pos"]) for x in result])
yticklabels = ", ".join([x["name"] for x in result])
plots = "\n\n".join([x["plot"] for x in result])
ymax = result[-1]["y_pos"] + 1

# generate the latex document
doc_file = os.path.join(tikz_folder, "plot.tex")

# generate the latex string
doc = rf"""
\documentclass{{standalone}}
\usepackage[english]{{babel}}

% tikz
\usepackage{{tikz}}
\usepackage{{pgfplots}}
\usetikzlibrary{{calc,shapes,arrows,decorations.markings}}
\usepgfplotslibrary{{fillbetween, groupplots, statistics}}

\pgfplotsset{{
  /pgfplots/boxplot legend/.style 2 args={{
    legend image code/.code={{
      \draw [|-|, #1] (2mm,0mm) -- node[rectangle,minimum size=2.5mm,#2]{{}} (7mm,0mm);
      \draw [#1] (4.3mm,-1.25mm) -- (4.3mm, 1.25mm);
    }}
  }}
}}
\pgfplotsset{{
  /pgfplots/minimum cost legend/.style 2 args={{
    legend image code/.code={{
     \draw[#1] (0mm, 0.8mm) rectangle (3mm, -0.8mm);
     \draw[#2] (3mm, 1.5mm) rectangle (3mm, -1.5mm);
    }}
  }}
}}

% colors
\usepackage{{xcolor}}
\definecolor{{cLightRed}}{{HTML}}{{E74C3C}}
\definecolor{{cRed}}{{HTML}}{{C0392B}}
\definecolor{{cBlue}}{{HTML}}{{2980B9}}
\definecolor{{cLightBlue}}{{HTML}}{{3498DB}}
\definecolor{{cDarkBlue}}{{HTML}}{{10334A}}
\definecolor{{cGreen}}{{HTML}}{{27AE60}}
\definecolor{{cLightGreen}}{{HTML}}{{2ECC71}}
\definecolor{{cViolet}}{{HTML}}{{8E44AD}}
\definecolor{{cLightViolet}}{{HTML}}{{9B59B6}}
\definecolor{{cOrange}}{{HTML}}{{D35400}}
\definecolor{{cLightOrange}}{{HTML}}{{E67E22}}
\definecolor{{cYellow}}{{HTML}}{{F39C12}}
\definecolor{{cLightYellow}}{{HTML}}{{F1C40F}}

\begin{{document}}
\begin{{tikzpicture}}
  \begin{{axis}}[
    height=20cm,
    width=6cm,
    ytick={{{ytick}}},
    yticklabels={{{yticklabels}}},
    yticklabel style={{font=\footnotesize, xshift=2ex}},
    xticklabel style={{font=\footnotesize}},
    axis y line*=left,
    axis x line*=bottom,
    y axis line style={{draw=none}},
    y tick style={{draw=none}},
    ymin=-1,
    ymax={ymax},
    xlabel={{Cost (traffic shifts)}},
    legend columns=3,
    legend style={{
      font=\footnotesize,
      anchor=south,
      at={{(0.5, 1.005)}},
      draw=none,
      /tikz/every even column/.append style={{column sep=0.2cm}},
    }},
    ]
    \addlegendimage{{boxplot legend={{draw=cRed}}{{draw=cRed, fill=cRed!20}}}}
    \addlegendentry{{Random order}}
    \addlegendimage{{minimum cost legend={{draw=none, fill=black!20}}{{draw=black}}}}
    \addlegendentry{{Ideal cost}}

    {plots}
  \end{{axis}}
\end{{tikzpicture}}
\end{{document}}"""

# write the data to disk
with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "plot_2.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
