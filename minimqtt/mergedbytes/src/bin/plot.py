import matplotlib.pyplot as plt
import numpy as np
from typing import List, Tuple
import sys


def plot(labels: List[str], values_with_label: List[Tuple[List[int], str]]):
    x = np.arange(len(labels))  # the label locations
    width = 0.2  # the width of the bars
    fig, ax = plt.subplots()

    for i, (values, label) in enumerate(values_with_label):
        rects1 = ax.barh(x + (i * width), values, width, label=label)
        autolabel(ax, rects1)

    # Add some text for labels, title and custom x-axis tick labels, etc.
    ax.set_xlabel('Throughput (messages/sec)')
    ax.set_yticks(x)
    ax.set_yticklabels(labels)
    ax.legend()

    fig.tight_layout()

    plt.show()


def autolabel(ax, rects):
    """
    Attach a text label above each bar displaying its height
    """
    for rect in rects:
        width = rect.get_width()
        ax.text(rect.get_x() + rect.get_width() / 2, rect.get_y() + rect.get_height()/2.,
                '%.2f' % width,
                ha='center', va='center', color='white')


tokio = ([1, 2, 3, 4, 5], 'tokio')
smol = ([6, 7, 8, 9, 10], 'smol')
async_std = ([11, 12, 13, 14, 15], 'async-std')

plot(['a', 'b', 'c', 'd', 'e'], [tokio, smol, async_std])

# results = []
# for f in sys.argv[1:]:
#     with open(f) as f:
#         for line in f.readlines():
#             test, lang, impl, secs, _ = line.split()
#             results.append((test, lang, impl, float(secs)))
