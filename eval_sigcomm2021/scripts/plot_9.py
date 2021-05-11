import json
import os
import shutil

import numpy as np
import pandas as pd
import sys

if "-pre" in sys.argv:
    root_folder = "eval_sigcomm2021/precomputed_results/result_9"
else:
    root_folder = "eval_sigcomm2021/result_9"

####################
# prepare the data #
####################


def file_to_boxplot(filename, key):
    with open(os.path.join(root_folder, filename), 'r') as fp:
        data = json.load(fp)
    states = np.array([x['num_states'] for x in data[key]])
    q = pd.DataFrame({'data': states}).quantile([0.05, 0.25, 0.5, 0.75, 0.95])
    return list(q.data)


networks = list({filename.split(".")[0]
                 for filename in os.listdir(root_folder)
                 if filename.endswith('json')})
networks.sort()

df = pd.DataFrame(columns=['network',
                           'random_05', 'random_25', 'random_75', 'random_95',
                           'optimizer_05', 'optimizer_25', 'optimizer_75', 'optimizer_95',
                           'strategy_05', 'strategy_25', 'strategy_75', 'strategy_95'])
for net in networks:
    (r05, r25, r50, r75, r95) = file_to_boxplot(f"{net}.gml.rand.json", 'random_result')
    (s05, s25, s50, s75, s95) = file_to_boxplot(f"{net}.gml.strat.json", 'strategy_result')
    (o05, o25, o50, o75, o95) = file_to_boxplot(f"{net}.gml.optim.json", 'strategy_result')

    df = df.append({'network': net,
                    'random_05': r05,
                    'random_25': r25,
                    'random_50': r50,
                    'random_75': r75,
                    'random_95': r95,
                    'optimizer_05': o05,
                    'optimizer_25': o25,
                    'optimizer_50': o50,
                    'optimizer_75': o75,
                    'optimizer_95': o95,
                    'strategy_05': s05,
                    'strategy_25': s25,
                    'strategy_50': s50,
                    'strategy_75': s75,
                    'strategy_95': s95}, ignore_index=True)

df = df.sort_values(by=['strategy_50']).reset_index(drop=True)

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
df.to_csv(csv_file, sep=',', index_label="idx")

# generate the latex document
doc_file = os.path.join(tikz_folder, "plot.tex")
doc = rf"""\documentclass{{standalone}}
\usepackage[english]{{babel}}

% tikz
\usepackage{{tikz}}
\usepackage{{pgfplots}}
\usetikzlibrary{{calc,shapes,arrows,decorations.markings}}
\usepgfplotslibrary{{fillbetween, groupplots, statistics}}

\pgfplotsset{{
  /pgfplots/evaluation line legend/.style 2 args={{
    legend image code/.code={{
      \draw[#2] (0mm, 1.5mm) rectangle (5mm, -1.5mm);
      \draw[#1] (0mm, 0mm) -- (5mm, 0mm);
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
    height=5cm,
    width=7cm,
    xtick = {{{len(networks) - 1}}},
    xticklabels = {{{len(networks)}}},
    axis x line = bottom,
    axis y line = left,
    xlabel = {{Networks from Topology Zoo}},
    x label style={{at={{(axis description cs:0.5, -0.03)}}, anchor=south}},
    ylabel = {{Explored states}},
    legend cell align=left,
    legend style={{
      draw=none,
      font=\small,
      anchor=west,
      at={{(1, 0.5)}},
    }},
    xmin=0,
    xmax={len(networks)},
    ymode=log,
    ]
    \addlegendimage{{evaluation line legend={{thick, cRed}}{{fill=cRed!25, draw=none}}}}
    \addlegendentry{{Random permutations}}

    \addlegendimage{{evaluation line legend={{thick, cBlue}}{{fill=cBlue!25, draw=none}}}}
    \addlegendentry{{Snowcap (hard spec. only)}}

    \addlegendimage{{evaluation line legend={{thick, cGreen}}{{fill=cGreen!25, draw=none}}}}
    \addlegendentry{{Snowcap}}

    \addplot+[mark=none, thick, cRed] table[x=idx, y=random_50, col sep=comma] {{data.csv}};
    \addplot+[mark=none, thick, cGreen] table[x=idx, y=optimizer_50, col sep=comma] {{data.csv}};
    \addplot+[mark=none, thick, cBlue] table[x=idx, y=strategy_50, col sep=comma] {{data.csv}};

    \addplot+[mark=none, draw=none, name path=r2] table[x=idx, y=random_25, col sep=comma]{{data.csv}};
    \addplot+[mark=none, draw=none, name path=r4] table[x=idx, y=random_75, col sep=comma]{{data.csv}};

    \addplot+[mark=none, draw=none, name path=o2] table[x=idx, y=strategy_25, col sep=comma]{{data.csv}};
    \addplot+[mark=none, draw=none, name path=o4] table[x=idx, y=strategy_75, col sep=comma]{{data.csv}};

    \addplot+[mark=none, draw=none, name path=s2] table[x=idx, y=optimizer_25, col sep=comma]{{data.csv}};
    \addplot+[mark=none, draw=none, name path=s4] table[x=idx, y=optimizer_75, col sep=comma]{{data.csv}};

    \addplot[draw=none, fill=cRed,   fill opacity=0.25] fill between[of=r2 and r4];
    \addplot[draw=none, fill=cGreen, fill opacity=0.25] fill between[of=o2 and o4];
    \addplot[draw=none, fill=cBlue,  fill opacity=0.25] fill between[of=s2 and s4];

  \end{{axis}}

\end{{tikzpicture}}
\end{{document}}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), "plot_9.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
