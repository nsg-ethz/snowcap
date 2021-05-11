import json
import os
import sys
import shutil
import pandas as pd
import numpy as np

if len(sys.argv) >= 2 and sys.argv[1] in {"6", "7", "8"}:
    eval_id = sys.argv[1]
else:
    raise RuntimeError("Run this script with the first argument either 6, 7 or 8")

if "-pre" in sys.argv:
    root_folder = f"eval_sigcomm2021/precomputed_results/result_{eval_id}"
else:
    root_folder = f"eval_sigcomm2021/result_{eval_id}"

####################
# prepare the data #
####################

def get_boxplot_data(data):
    q = pd.DataFrame({'data': data}).quantile([0.05, 0.25, 0.5, 0.75, 0.95])
    return list(q.data)

# read the data
df = pd.DataFrame(columns=['network', 'optimal', 'fraction',
                           'random_05', 'random_25', 'random_50', 'random_75', 'random_95',
                           'mif_05', 'mif_25', 'mif_50', 'mif_75', 'mif_95',
                           'mil_05', 'mil_25', 'mil_50', 'mil_75', 'mil_95',
                           'snowcap_05', 'snowcap_25', 'snowcap_50', 'snowcap_75', 'snowcap_95'])
num_networks = 0

for filename in os.listdir(root_folder):
    if not filename.endswith(".json"):
        continue

    # read the file
    filepath = os.path.join(root_folder, filename)
    with open(filepath, 'r') as fp:
        data = json.load(fp)

    # extract the relevant data
    random = np.array([np.float64(x["cost"]) for x in data["random_result"]])
    snowcap = np.array([np.float64(x["cost"]) for x in data["strategy_result"]])
    mif = np.array([np.float64(x["cost"]) for x in data["baseline_mif_result"]])
    mil = np.array([np.float64(x["cost"]) for x in data["baseline_mil_result"]])
    ideal_cost = data["ideal_cost"]

    # check the data
    if np.any(np.isnan(random)):
        print(f"{filename}: Random method has no solution! skipping {filename}!")
        continue
    if np.any(np.isnan(snowcap)):
        raise RuntimeError(f"{filename}: Snowcap method has no solution! Abort!")
    if np.any(np.isnan(mif)):
        print(f"{filename}: Most-Important-Last method has no solution! skipping {filename}!")
        continue
    if np.any(np.isnan(mil)):
        print(f"{filename}: Most-Important-First method has no solution! skipping {filename}!")
        continue

    # prepare the boxplot
    (r05, r25, r50, r75, r95) = get_boxplot_data(random)
    (s05, s25, s50, s75, s95) = get_boxplot_data(snowcap)
    (f05, f25, f50, f75, f95) = get_boxplot_data(mif)
    (l05, l25, l50, l75, l95) = get_boxplot_data(mil)

    # prepare the CDF data
    if r50 == s50:
        fraction = 1
    elif r50 == 0:
        fraction = 1000000
    else:
        fraction = s50 / r50

    df = df.append({'network': filename.split(".")[0],
                    'optimal': ideal_cost,
                    'fraction': fraction,
                    'random_05': r05,
                    'random_25': r25,
                    'random_50': r50,
                    'random_75': r75,
                    'random_95': r95,
                    'mif_05': f05,
                    'mif_25': f25,
                    'mif_50': f50,
                    'mif_75': f75,
                    'mif_95': f95,
                    'mil_05': l05,
                    'mil_25': l25,
                    'mil_50': l50,
                    'mil_75': l75,
                    'mil_95': l95,
                    'snowcap_05': s05,
                    'snowcap_25': s25,
                    'snowcap_50': s50,
                    'snowcap_75': s75,
                    'snowcap_95': s95}, ignore_index=True)

    num_networks += 1

# sort the data along random_50
df.sort_values("random_50", ignore_index=True, inplace=True)

########################
# Prepare the CDF plot #
########################

mid_point = np.sum(df['fraction'] <= 0.5) / num_networks
zero_point = np.sum(df['fraction'] <= 0) / num_networks
if zero_point > 0.1:
    zero_point_tick = f", {zero_point}"
    zero_point_tick_label = f", {zero_point * 100:.1f}\%"
else:
    zero_point_tick = ""
    zero_point_tick_label = ""

if np.all(df['fraction'] <= 1):
    max_x = 1.1
    max_val_tick = ""
    max_val_tick_label = ""
    cdf_plot = rf"""
    \addplot+[hist={{bins=500, cumulative=true, density=true, data max=1.1}}, draw=none, fill=cGreen!60, mark=none]
    table [y=fraction, col sep=comma] {{data.csv}};
    \draw[draw=none, fill=black!10] (axis cs:1, 0) rectangle (axis cs:1.1, 1);
    \addplot+[hist={{bins=500, cumulative=true, density=true, data max=1.1, intervals=false, handler/.style={{sharp plot}}}}, draw=cGreen, fill=none, mark=none, thick]
    table [y=fraction, col sep=comma] {{data.csv}};
"""
else:
    print("\n\n\n\n\nSPECIAL\n\n\n\n\n\n")
    max_val = np.max(df['fraction'])
    max_val_tick = f", {max_val}"
    max_val_tick_label = f", {max_val:.2f}"
    fraction_less = np.sum(df['fraction'] <= 1) / num_networks
    max_x = max_val + 0.05
    cdf_plot = rf"""
    \addplot+[hist={{bins=500, cumulative=true, density=true, data max={max_x}}}, draw=none, fill=cGreen!60, mark=none]
    table [y=fraction, col sep=comma] {{data.csv}};
    \begin{{scope}}
      \clip (axis cs:0, {fraction_less}) rectangle (axis cs:{max_x + 0.1}, 1.1);
      \addplot+[hist={{bins=500, cumulative=true, density=true, data max={max_x}}}, draw=none, fill=cRed!60, mark=none]
      table [y=fraction, col sep=comma] {{data.csv}};
    \end{{scope}}
    \draw[draw=none, fill=black!10] (axis cs:1, 0) rectangle (axis cs:{max_x}, {fraction_less});
    \begin{{scope}}
      \clip (axis cs:0, 0) rectangle (axis cs:1, 1.1);
      \addplot+[hist={{bins=500, cumulative=true, density=true, data max={max_x}, intervals=false, handler/.style={{sharp plot}}}}, draw=cGreen, fill=none, mark=none, thick]
      table [y=fraction, col sep=comma] {{data.csv}};
    \end{{scope}}
    \begin{{scope}}
      \clip (axis cs:1, 0) rectangle (axis cs:{max_x}, 1.1);
      \addplot+[hist={{bins=500, cumulative=true, density=true, data max={max_x}, intervals=false, handler/.style={{sharp plot}}}}, draw=cRed, fill=none, mark=none, thick]
      table [y=fraction, col sep=comma] {{data.csv}};
    \end{{scope}}
    \draw[dashed] (axis cs:0, {fraction_less}) -- (axis cs:{max_x}, {fraction_less});
    \draw[<-, >=latex] (axis cs:0, {fraction_less}) -- (axis cs:0.1, 0.85) node[right] {{{fraction_less * 100:.1f}\%}};
"""

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
df.to_csv(csv_file, sep=',', index_label="pos")

# generate the latex document
doc_file = os.path.join(tikz_folder, "plot.tex")
doc = rf"""\documentclass[varwidth]{{standalone}}
\usepackage[english]{{babel}}
\usepackage{{amsmath}}

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
    axis x line*=bottom,
    x axis line style={{draw=none}},
    axis y line=left,
    xmin=0,
    xmax={num_networks - 1},
    ymin=0,
    axis x line=bottom,
    axis y line=left,
    ylabel={{Cost (Traffic Shift)}},
    xtick={{{num_networks - 1}}},
    xticklabels={{{num_networks}}},
    legend cell align=left,
    legend style={{
      draw=none,
      font=\small,
      anchor=west,
      at={{(1, 0.5)}},
    }},
    ]

    \addlegendimage{{evaluation line legend={{thick, cViolet, dashed, dash pattern=on 1pt off 1pt}}{{fill=white, draw=none}}}}
    \addlegendentry{{Baseline: important first}}

    \addlegendimage{{evaluation line legend={{thick, cYellow, dashed, dash pattern=on 1pt off 1pt}}{{fill=white, draw=none}}}}
    \addlegendentry{{Baseline: important last}}

    \addlegendimage{{evaluation line legend={{thick, cRed}}{{fill=cRed!25, draw=none}}}}
    \addlegendentry{{Baseline: random}}

    \addlegendimage{{evaluation line legend={{thick, cGreen}}{{fill=cGreen!25, draw=none}}}}
    \addlegendentry{{Snowcap}}

    \addplot+[mark=none, cViolet, thick, dashed, dash pattern=on 1pt off 1pt] table[x=pos, y=mif_50, col sep=comma] {{data.csv}};
    \addplot+[mark=none, cYellow, thick, dashed, dash pattern=on 1pt off 1pt] table[x=pos, y=mil_50, col sep=comma] {{data.csv}};
    \addplot+[mark=none, thick, cRed] table[x=pos, y=random_50, col sep=comma] {{data.csv}};
    \addplot+[mark=none, thick, cGreen] table[x=pos, y=snowcap_50, col sep=comma] {{data.csv}};

    \addplot+[mark=none, draw=none, name path=r2] table[x=pos, y=random_25, col sep=comma]{{data.csv}};
    \addplot+[mark=none, draw=none, name path=r4] table[x=pos, y=random_75, col sep=comma]{{data.csv}};

    \addplot+[mark=none, draw=none, name path=s2] table[x=pos, y=snowcap_25, col sep=comma]{{data.csv}};
    \addplot+[mark=none, draw=none, name path=s4] table[x=pos, y=snowcap_75, col sep=comma]{{data.csv}};

    \addplot[draw=none, fill=cRed,   fill opacity=0.25] fill between[of=r2 and r4];
    \addplot[draw=none, fill=cGreen, fill opacity=0.25] fill between[of=s2 and s4];

  \end{{axis}}
\end{{tikzpicture}}

\vspace{{1em}}

\begin{{tikzpicture}}
  \begin{{axis}}[
    axis x line=bottom,
    axis y line=left,
    height=4cm,
    width=7cm,
    ymin=0,
    ymax=1.1,
    xmin=0,
    xmax={max_x + 0.05},
    xtick={{0, 0.5, 1{max_val_tick}}},
    xticklabels={{0, 0.5, 1{max_val_tick_label}}},
    ytick={{0{zero_point_tick}, {mid_point}, 1}},
    yticklabels={{0\%{zero_point_tick_label}, {mid_point * 100:.1f}\%, 100\%}},
    xlabel={{$\text{{Cost}}_{{\text{{snowcap}}}} / \text{{Cost}}_{{\text{{Random}}}}$}},
    ylabel={{CDF}},
    y label style={{at={{(axis description cs:-0.07, 0.5)}}}},
    clip=false,
    ]
    {cdf_plot}
    \draw[dashed] (axis cs:0, {mid_point}) -- (axis cs:{max_x}, {mid_point});
    \draw[dashed] (axis cs:0.5, 0) -- (axis cs:0.5, 1.1);
  \end{{axis}}
\end{{tikzpicture}}
\end{{document}}"""

with open(doc_file, 'w') as fp:
    fp.write(doc)

# compile the document
os.chdir(tikz_folder)
os.system("pdflatex plot.tex")
plot_pdf_file = os.path.join(tikz_folder, "plot.pdf")
result_pdf_file = os.path.join(os.path.dirname(root_folder), f"plot_{eval_id}.pdf")
for _ in plot_pdf_file.split("/")[1:]:
    os.chdir("..")
shutil.copy(plot_pdf_file, result_pdf_file)

print(f"\n\nGenerated plot at: {result_pdf_file}")
