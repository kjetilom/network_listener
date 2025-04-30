import os
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt
import sqlalchemy
from sqlalchemy import create_engine
import psycopg2
from matplotlib.lines import Line2D
from matplotlib.patches import Patch
from matplotlib.patches import Circle
from seaborn.objects import Dot
import numpy as np
import statsmodels.api as sm
from joblib import Parallel, delayed

sns.set_style("whitegrid")
sns.set_context("paper", font_scale=2)

experiments_db = {
    "exp1": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp1_fluid": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp1_jitter": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp2": {
        "capacity": 3000000,
        "max_capacity": 5000000,
    },
    "exp2_fluid": {
        "capacity": 3000000,
        "max_capacity": 5000000,
    },
    "exp3": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp4": {
        "capacity": 1000000,
        "max_capacity": 4000000,
    },
    "exp5": {
        "capacity": 1000000,
        "max_capacity": 4000000,
    },
    "exp6": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp7": {
        "capacity": 5000000,
        "max_capacity": 10000000,
    },
    "exp8": {
        "capacity": 7000000,
        "max_capacity": 10000000,
    }
}

# experiments = {
#     "exp1": {
#         "link_states": "experiments/exp1/exp1_16_apr_2025/link_states_exp1.csv",
#         "throughput_dps": "experiments/exp1/exp1_16_apr_2025/throughput.csv",
#         "pgm_dps": "experiments/exp1/exp1_16_apr_2025/pgm_dps_exp1.csv",
#         "capacity": 5000000,
#         "max_capacity": 10000000,
#         "main_node": "n2",
#         "udp_throughput": 1213735,
#     },
#     "exp1_fluid": {
#         "link_states": "experiments/exp1_fluid/exp1_fluid_16_apr_2025/link_states_exp1_fluid.csv",
#         "throughput_dps": "experiments/exp1_fluid/exp1_fluid_16_apr_2025/throughput.csv",
#         "pgm_dps": "experiments/exp1_fluid/exp1_fluid_16_apr_2025/pgm_dps_exp1_fluid.csv",
#         "capacity": 5000000,
#         "max_capacity": 10000000,
#         "main_node": "n2",
#         "udp_throughput": 2240893,
#     },
#     "exp2": {
#         "link_states": "experiments/exp2/exp2_16_apr_2025/link_states_exp2.csv",
#         "throughput_dps": "experiments/exp2/exp2_16_apr_2025/throughput.csv",
#         "pgm_dps": "experiments/exp2/exp2_16_apr_2025/pgm_dps_exp2.csv",
#         "capacity": 3000000,
#         "max_capacity": 5000000,
#         "main_node": "n2",
#         "udp_throughput": 795973,
#     },
#     "exp3": {
#         "link_states": "experiments/exp3/exp3_11apr_2025/link_states_exp3.csv",
#         "throughput_dps": "experiments/exp3/exp3_11apr_2025/throughput_dps_exp3.2.csv",
#         "pgm_dps": "experiments/exp3/exp3_11apr_2025/pgm_dps_exp3.csv",
#         "capacity": 5000000,
#         "max_capacity": 10000000,
#         "main_node": "n2",
#         "udp_throughput": 1900603,
#     },
#     "exp4": {
#         "link_states": "experiments/exp4/exp4_2025_13_04/link_states_exp4.csv",
#         "throughput_dps": "experiments/exp4/exp4_2025_13_04/throughput.csv",
#         "pgm_dps": "experiments/exp4/exp4_2025_13_04/pgm_dps_exp4.csv",
#         "capacity": 1000000,
#         "max_capacity": 4000000,
#         "main_node": "n2",
#         "udp_throughput": 327750,
#     },
#     "exp5": {
#         "link_states": "experiments/exp5/exp5_16_apr_2025/link_states_exp5.csv",
#         "throughput_dps": "experiments/exp5/exp5_16_apr_2025/throughput.csv",
#         "pgm_dps": "experiments/exp5/exp5_16_apr_2025/pgm_dps_exp5.csv",
#         "capacity": 1000000,
#         "max_capacity": 4000000,
#         "main_node": "n2",
#         "udp_throughput": 526546,
#     },
#     "exp6": {
#         "link_states": "experiments/exp6/exp6_21_apr_2025/link_states_exp6.csv",
#         "throughput_dps": "experiments/exp6/exp6_21_apr_2025/throughput.csv",
#         "pgm_dps": "experiments/exp6/exp6_21_apr_2025/pgm_dps_exp6.csv",
#         "capacity": 5000000,
#         "max_capacity": 10000000,
#         "main_node": "n2",
#         "udp_throughput": 1234567,
#     },
# }


def read_data(file_path):
    """
    Reads a CSV file and returns a DataFrame.
    """
    return pd.read_csv(file_path, delimiter=",", header="infer", encoding="utf-8")


def read_data_with_header(file_path):
    """
    Reads a CSV file and returns a DataFrame with the first row as the header.
    """
    return pd.read_csv(file_path, delimiter=",", header=0, encoding="utf-8")


# def group_by_sender_receiver(df):
#     """
#        sender_ip   receiver_ip   gin      gout    len   num_acked  time
#     0  10.0.1.20   10.0.2.20  0.029314  0.029340  1448          1  2025-03-28 18:34:23.509+00
#     1  10.0.1.20   10.0.2.20  0.002412  0.002416  1448          1  2025-03-28 18:34:23.509+00

#         Groups the DataFrame by sender_ip and receiver_ip.
#         returns a list of dataframes, one for each group
#     """
#     grouped = df.groupby(["sender_ip", "receiver_ip"])
#     dataframes = [group for _, group in grouped]
#     return dataframes


# def gin_gout_to_dps(df):
#     """
#     Converts gin and gout columns to datapoints for regression.
#     gout/gin, len/gin
#     """
#     df["gout/gin"] = df["gout"] / df["gin"]
#     df["len/gin"] = df["len"] / df["gin"]


# def filter_df(df, max_thp):
#     """
#     Filters the DataFrame based on a maximum throughput value.
#     """
#     df = df[df["len/gin"] < max_thp]
#     df = df[df["len"] / df["gout"] < max_thp]
#     return df


# def filter_detailed(df: pd.DataFrame):
#     """
#     Filters the DataFrame to remove outliers based on the 10th percentile of the gout column.
#     """
#     max_gin_10p = df["gout"].quantile(0.1)
#     unused = df[df["gin"] > max_gin_10p]
#     df = df[df["gin"] < max_gin_10p]
#     df["used"] = True
#     unused["used"] = False
#     return df, unused


# def plot_scatter(df, x_col, y_col):
#     """
#     Plots a scatter plot of the given columns.
#     """
#     f, ax = plt.subplots(figsize=(10, 10))
#     sns.despine(f, left=True, bottom=True)
#     df[x_col] = df[x_col] / 125000

#     sns.scatterplot(
#         data=df, x=x_col, y=y_col, ax=ax, color="k", alpha=0.8, size=0.25, legend=False
#     )
#     sns.kdeplot(
#         data=df, x=x_col, y=y_col, fill=True, alpha=0.6, cut=0.1, levels=20, ax=ax
#     )

#     ax.set(xlabel="Mbit/s", ylabel="gout/gin")
#     # ax.set_xscale('log')
#     # ax.set_yscale('log')
#     plt.title(f"Relationship between {x_col} and {y_col}")
#     plt.tight_layout()
#     plt.show()


# def plot_regressin(
#     df, df_unused, x_col, y_col, robust, max_capacity, experiment, capacity
# ):
#     """
#     Plots a regression plot of the given columns.
#     """
#     f, ax = plt.subplots(figsize=(20, 10))
#     # sns.despine(f, left=True, bottom=True)
#     df[x_col] = df[x_col] / 125000
#     df_unused[x_col] = df_unused[x_col] / 125000

#     sns.regplot(
#         robust=robust,
#         data=df,
#         x=x_col,
#         y=y_col,
#         ax=ax,
#         scatter=True,
#         scatter_kws={"s": 12, "alpha": 0.8},
#         line_kws={"color": "red"},
#         ci=None,
#         truncate=False,
#     )
#     sns.scatterplot(
#         data=df_unused,
#         x=x_col,
#         y=y_col,
#         ax=ax,
#         color="k",
#         alpha=0.8,
#         s=12,
#         legend=False,
#     )
#     ax.axvline(
#         x=capacity / 1000000 * 0.965333333,
#         color="blue",
#         linestyle="--",
#         label="Capacity",
#     )
#     ax.set(xlabel="Mbit/s (len/gin)", ylabel="gout/gin")
#     # plt.xlim(0, max_capacity//1000000)
#     # plt.ylim(0, 5)
#     ax.set_xscale("log")
#     ax.set_yscale("log")
#     plt.legend(
#         ["Used for regression", "Regression line", "Filtered out"], loc="upper right"
#     )
#     plt.title(f"Regression of {x_col} and {y_col} ({experiment})")
#     plt.tight_layout()
#     plt.show()


# def plot_regression(df, df_unused, x_col, y_col, capacity):
#     # Plot side by side
#     f, (ax1, ax2) = plt.subplots(1, 2, figsize=(20, 10))
#     df[x_col] = df[x_col] / 125000
#     df_unused[x_col] = df_unused[x_col] / 125000
#     # Perform regression using robust linear regression
#     sns.regplot(
#         robust=True,
#         data=df,
#         x=x_col,
#         y=y_col,
#         ax=ax1,
#         scatter=True,
#         scatter_kws={"s": 3, "alpha": 0.8},
#         line_kws={"color": "red"},
#         ci=None,
#     )
#     sns.scatterplot(
#         data=df_unused,
#         x=x_col,
#         y=y_col,
#         ax=ax1,
#         color="k",
#         alpha=0.8,
#         s=3,
#         legend=False,
#     )
#     ax1.set(xlabel="Mbit/s", ylabel="gout/gin")

#     sns.regplot(
#         data=df,
#         x=x_col,
#         y=y_col,
#         ax=ax2,
#         scatter=True,
#         scatter_kws={"s": 4, "alpha": 0.8},
#         line_kws={"color": "red"},
#         ci=None,
#     )
#     sns.scatterplot(
#         data=df_unused,
#         x=x_col,
#         y=y_col,
#         ax=ax2,
#         color="k",
#         alpha=0.8,
#         s=4,
#         legend=False,
#     )
#     ax2.set(xlabel="Mbit/s", ylabel="gout/gin")
#     plt.legend(
#         ["Used for regression", "Regression line", "Filtered out"], loc="upper right"
#     )
#     plt.title(f"Regression of {x_col} and {y_col}")
#     plt.tight_layout()

#     plt.show()


# def plot_timeseries(df, x_col, y_col):
#     """
#     Plots a time series plot of the given columns.
#     """
#     f, ax = plt.subplots(figsize=(20, 10))
#     sns.despine(f, left=True, bottom=True)
#     df["ip_pair"] = df["sender_ip"] + " -> " + df["receiver_ip"]

#     sns.lineplot(
#         data=df,
#         x=x_col,
#         y=y_col,
#         ax=ax,
#         color="k",
#         alpha=0.8,
#         hue="ip_pair",
#         legend=False,
#     )
#     plt.title(f"Time series of {x_col} and {y_col}")
#     plt.tight_layout()
#     plt.show()


# def process_ip_pair(group, throughput_dps):
#     # Only process groups with enough data

#     # Get sender and receiver subnets from the group.
#     snd_subnet = group["snd_subnet"].iloc[0]
#     rcv_subnet = group["rcv_subnet"].iloc[0]
#     ip_pair_label = f"{group['sender_ip'].iloc[0]} - {group['receiver_ip'].iloc[0]}"

#     # Filter throughput data for each subnet.
#     throughput1 = throughput_dps[throughput_dps["subnet"] == rcv_subnet]
#     throughput2 = throughput_dps[throughput_dps["subnet"] == snd_subnet]

#     # Merge the throughput data on time and compute the minimum ABW from the two.
#     merged_tp = throughput1.merge(throughput2, on="time", suffixes=("_1", "_2"))
#     merged_tp["real_abw"] = merged_tp[["real_abw_1", "real_abw_2"]].min(axis=1)
#     mean_abw = merged_tp["real_abw"].median()
#     # Compute rolling median on the merged throughput data (using a window of 35 values)
#     merged_tp["abw_rolling_mean"] = merged_tp["real_abw"].rolling(window=35).mean()

#     # Ensure sorting by time before merging
#     group_sorted = group.sort_values("time")
#     merged_tp_sorted = merged_tp.sort_values("time")

#     # Create a time-indexed series from the merged throughput data
#     abw_series = merged_tp_sorted.set_index("time")["abw_rolling_mean"]

#     abw_series = abw_series[~abw_series.index.duplicated(keep="first")]
#     # First, form a union index so that both series are aligned, then reindex and interpolate:
#     union_index = abw_series.index.union(group_sorted["time"])
#     abw_series_reindexed = abw_series.reindex(union_index)
#     abw_series_interp = abw_series_reindexed.interpolate(method="time")
#     # Preview the interpolated series at the group times:
#     interp_values = abw_series_interp.reindex(group_sorted["time"])
#     # print(interp_values.head())
#     # Copy group_sorted to avoid modifying the original dataframe and assign the interpolated values:
#     group_sorted = group_sorted.copy()
#     group_sorted["abw_rolling_mean_interpolated"] = interp_values.values

#     # Optionally, drop rows with NaN in case the group's times fall outside the interpolation range:
#     group_sorted = group_sorted.dropna(subset=["abw_rolling_mean_interpolated"])

#     # Calculate error as (estimated ABW - real ABW [interpolated rolling median])
#     group_sorted["error"] = (
#         group_sorted["abw"] - group_sorted["abw_rolling_mean_interpolated"]
#     )
#     group_sorted["error_expected"] = group_sorted["abw"] - mean_abw
#     group_sorted["ip_pair"] = ip_pair_label

#     # # Use merge_asof to align estimated ABW (from link_states group) with real ABW (rolling median)
#     # merged_data = pd.merge_asof(
#     #     group_sorted, merged_tp_sorted,
#     #     on="time", direction="backward", tolerance=pd.Timedelta("5s")
#     # )
#     # merged_data = merged_data.dropna(subset=["abw_rolling_mean"])

#     # # Calculate error as (estimated ABW - real ABW [rolling median])
#     # merged_data["error"] = merged_data["abw"] - merged_data["abw_rolling_mean"]
#     # merged_data["ip_pair"] = ip_pair_label

#     # Return only the necessary columns.
#     return group_sorted


# def prepare_error_data(link_states, throughput_dps, capacity, main_node):
#     """ """
#     link_states, throughput_dps = prepare_abw_and_throughput_data(
#         link_states, throughput_dps, capacity, main_node
#     )

#     # Calculate the error based on the expected abw.

#     # Create merged dataframe with link_states and throughput_dps
#     grouped = link_states.groupby(["sender_ip", "receiver_ip"])

#     # errors = grouped.apply(lambda group: process_ip_pair(group, throughput_dps), include_groups=False)
#     errors = pd.DataFrame()
#     for _, group in grouped:
#         error = process_ip_pair(group, throughput_dps)
#         errors = pd.concat([errors, error], ignore_index=True)

#     return errors.reset_index(drop=True)


# def prepare_abw_and_throughput_data(
#     link_states_path, throughput_dps_path, capacity, main_node
# ):
#     """
#     Prepares the ABW and throughput data for analysis.
#     """
#     # Read the data files.
#     link_states = read_data_with_header(link_states_path)
#     throughput_dps = read_data_with_header(throughput_dps_path)

#     # Convert time to datetime.
#     link_states["time"] = pd.to_datetime(link_states["time"])
#     throughput_dps["time"] = pd.to_datetime(throughput_dps["time"], unit="ms", utc=True)
#     throughput_dps = throughput_dps[
#         (throughput_dps["node1"] == main_node) | (throughput_dps["node2"] == main_node)
#     ]

#     throughput_dps.drop_duplicates(subset=["time", "throughput"], inplace=True)

#     throughput_dps["real_abw"] = capacity - throughput_dps["throughput"]

#     # Merge ip41 and ip42 columns into one column
#     throughput_dps["ip41"] = throughput_dps["ip41"].combine_first(
#         throughput_dps["ip42"]
#     )
#     throughput_dps.drop(columns=["ip42"], inplace=True)
#     throughput_dps.rename(columns={"ip41": "ip"}, inplace=True)

#     # Create subnet and interface fields
#     throughput_dps["subnet"] = throughput_dps["ip"].str.split(".").str[:3].str.join(".")
#     throughput_dps["iface"] = throughput_dps["iface1"].combine_first(
#         throughput_dps["iface2"]
#     )
#     throughput_dps.drop(columns=["iface1", "iface2"], inplace=True)

#     link_states["rcv_subnet"] = (
#         link_states["receiver_ip"].str.split(".").str[:3].str.join(".")
#     )
#     link_states["snd_subnet"] = (
#         link_states["sender_ip"].str.split(".").str[:3].str.join(".")
#     )
#     # Convert estimated ABW to bits per second if needed.
#     link_states["abw"] *= 8
#     # Remove rows with abw == 0 in link_states
#     link_states = link_states[link_states["abw"] != 0]

#     return link_states, throughput_dps


# def prepare_pgm_data(exp):
#     complete_data = []
#     for exp_name, exp_data in exp.items():
#         pgm_dps = read_data_with_header(exp_data["pgm_dps"])
#         pgm_dps["time"] = pd.to_datetime(pgm_dps["time"])
#         pgm_dps["experiment"] = exp_name
#         pgm_dps["ip_pair"] = pgm_dps["sender_ip"] + " -> " + pgm_dps["receiver_ip"]
#         gin_gout_to_dps(pgm_dps)
#         grouped_data = group_by_sender_receiver(pgm_dps)
#         grouped_data = [group for group in grouped_data if group.size > 100]

#         grouped_data = [
#             filter_df(group, exp_data["max_capacity"] // 8) for group in grouped_data
#         ]
#         grouped_data = [filter_detailed(group) for group in grouped_data]

#         if len(grouped_data) == 0:
#             continue
#         combined_data = pd.concat(
#             [
#                 pd.concat([group_used, group_unused], ignore_index=True)
#                 for group_used, group_unused in grouped_data
#             ],
#             ignore_index=True,
#         )
#         complete_data.append(combined_data)

#     pgm_dps = pd.concat(complete_data, ignore_index=True)
#     pgm_dps["len/gin"] = pgm_dps["len/gin"] / 125000
#     for pair, group in pgm_dps.groupby(["ip_pair"]):
#         g = sns.FacetGrid(
#             group,
#             hue="experiment",
#             col="experiment",
#             col_wrap=3,
#             height=4,
#             sharex=False,
#         )
#         g.set(ylim=(0, 5))
#         g.map_dataframe(
#             plot_scatter_pgm, x="len/gin", y="gout/gin", alpha=0.8, legend=False, s=12
#         )

#         plt.show()

#         # for i, (group_used, group_unused) in enumerate(grouped_data):
#         #     plot_regressin(group_used, group_unused, 'len/gin','gout/gin', False, exp_data["max_capacity"], exp_name, exp_data["capacity"])


# def prepare_exp_errors(exp):
#     """
#     example:
#     "exp5": {
#             "link_states": "experiments/exp5/exp5_16_apr_2025/link_states_exp5.csv",
#             "throughput_dps": "experiments/exp5/exp5_16_apr_2025/throughput.csv",
#             "pgm_dps": "experiments/exp5/exp5_16_apr_2025/pgm_dps_exp5.csv",
#             "capacity": 1000000,
#             "main_node": "n2",
#             "udp_throughput": 526546
#         },
#     """
#     errors = {}
#     for exp_name, exp_data in exp.items():
#         errors[exp_name] = prepare_error_data(
#             exp_data["link_states"],
#             exp_data["throughput_dps"],
#             capacity=exp_data["capacity"],
#             main_node=exp_data["main_node"],
#         )
#         errors[exp_name]["error"] = (
#             errors[exp_name]["error"] / exp_data["capacity"] * 100
#         )
#         errors[exp_name]["error_expected"] = (
#             errors[exp_name]["error_expected"] / exp_data["capacity"] * 100
#         )
#     return errors


# def plot_scatter_pgm(data, **kwargs):
#     scatter1 = data[data["used"]]
#     scatter2 = data[data["used"] == False]
#     sns.scatterplot(data=scatter1, **kwargs)
#     kwargs["color"] = "k"
#     sns.scatterplot(data=scatter2, **kwargs)


# def boxplots(exp):
#     combined_errors = prepare_exp_errors(exp)

#     # Combine all errors into a single DataFrame
#     for key, value in combined_errors.items():
#         value["experiment"] = key
#     combined_errors = pd.concat(combined_errors.values(), ignore_index=True)

#     # combined_errors["error"] = abs(combined_errors["error"])
#     # combined_errors["error_expected"] = abs(combined_errors["error_expected"])

#     # Add separate plot for error_expected

#     # Plot the boxplot for all experiments
#     g = sns.PairGrid(
#         combined_errors,
#         y_vars=["error"],
#         x_vars=["experiment"],
#         aspect=2,
#         hue="experiment",
#     )
#     # g.map(sns.boxplot)
#     sns.boxplot(
#         data=combined_errors, x="experiment", y="error", ax=g.axes[0, 0], palette="Set2"
#     )
#     g.set(title="Available Bandwidth error by experiment")
#     # Add legend with experiment names, colors and the median error

#     plt.ylabel("Error (% of capacity)")
#     plt.xlabel("Experiment")
#     plt.ylim(top=200)
#     # plt.xticks(rotation=45)  # Rotate labels if many IP pairs are present.
#     plt.tight_layout()
#     plt.show()


# def probe_gap_plots(exp):
#     prepare_pgm_data(exp)


# def plot_median(data, **kwargs):
#     m = data.median()
#     plt.axhline(m, **kwargs)


# def timeseries_plots(exp):
#     exp_data = {}
#     for exp_name, data in exp.items():
#         link_states, throughput_dps = prepare_abw_and_throughput_data(
#             data["link_states"],
#             data["throughput_dps"],
#             capacity=data["capacity"],
#             main_node=data["main_node"],
#         )
#         link_states["experiment"] = exp_name
#         exp_data[exp_name] = {
#             "link_states": link_states,
#             "throughput_dps": throughput_dps,
#             "capacity": data["capacity"],
#             "max_capacity": data["max_capacity"],
#             "main_node": data["main_node"],
#             "udp_throughput": data["udp_throughput"],
#         }
#         # Add ip_pair
#         exp_data[exp_name]["link_states"]["ip_pair"] = (
#             exp_data[exp_name]["link_states"]["sender_ip"]
#             + " -> "
#             + exp_data[exp_name]["link_states"]["receiver_ip"]
#         )
#         # Convert time to relative time from 0 to the end of the experiment
#         exp_data[exp_name]["link_states"]["time"] = (
#             exp_data[exp_name]["link_states"]["time"]
#             - exp_data[exp_name]["link_states"]["time"].min()
#         ).dt.total_seconds()

#     # Combine all experiments into a single DataFrame
#     exp_data = pd.concat(
#         [data["link_states"] for data in exp_data.values()], ignore_index=True
#     )
#     exp_data["abw"] = exp_data["abw"] / 1000000  # Convert to Mbit/s

#     for ip_pair, group in exp_data.groupby("ip_pair"):
#         g = sns.FacetGrid(
#             group,
#             col="experiment",
#             hue="experiment",
#             col_wrap=3,
#             height=4,
#             aspect=1.5,
#             sharey=False,
#             sharex=False,
#         )
#         g.map(
#             sns.lineplot,
#             "time",
#             "abw",
#             alpha=0.8,
#             legend=False,
#         )
#         g.map(plot_median, "abw", color="red", linestyle="--")
#         g.set_axis_labels("Time (s)", "ABW (Mbit/s)")
#         g.set(xlim=(0, 3000))
#         plt.show()


def read_experiment(engine, exp_name):
    """
    Reads the experiment data from the database.
    """
    experiment = pd.read_sql_query(
        f"SELECT * FROM experiment WHERE name = '{exp_name}'", engine
    )
    if experiment.empty:
        raise ValueError(f"Experiment {exp_name} not found in the database.")
    experiment_id = experiment.iloc[0]["id"]

    pgm = pd.read_sql_query(
        f"SELECT * FROM pgm_detailed WHERE experiment_id = '{experiment_id}'", engine
    )
    link_states = pd.read_sql_query(
        f"SELECT * FROM link_states WHERE experiment_id = '{experiment_id}'", engine
    )
    throughputs = pd.read_sql_query(
        f"SELECT * FROM throughputs_filtered WHERE experiment_id = '{experiment_id}'",
        engine,
    )

    return pgm, link_states, throughputs


def get_pgm_filtered(engine, exp_name, capacity, max_capacity):
    """
    Gets the filtered PGM data from the database.
    """
    experiment = pd.read_sql_query(
        f"SELECT * FROM experiment WHERE name = '{exp_name}'", engine
    )
    if experiment.empty:
        raise ValueError(f"Experiment {exp_name} not found in the database.")
    experiment_id = experiment.iloc[0]["id"]

    query = f"""
SELECT *
FROM get_regression_candidates(
  {max_capacity / 8}::double precision,
  {1362}   ::double precision,
  {experiment_id},
  0.1
);
"""
    ret = pd.read_sql_query(query, engine)
    return ret


def pgm_filtered_by_timestamp(engine, exp_name, max_capacity):
    """
    Gets the filtered PGM data from the database.
    """
    experiment = pd.read_sql_query(
        f"SELECT * FROM experiment WHERE name = '{exp_name}'", engine
    )
    if experiment.empty:
        raise ValueError(f"Experiment {exp_name} not found in the database.")
    experiment_id = experiment.iloc[0]["id"]

    query = f"""
SELECT *
FROM get_regression_by_timestamp(
    {max_capacity / 8}::double precision,
    {1362}   ::double precision,
    {experiment_id},
    0.1
);
"""
    ret = pd.read_sql_query(query, engine)
    return ret


def get_real_abw(engine, exp_name, capacity):
    """
    Gets the real ABW from the database.
    """
    experiment = pd.read_sql_query(
        f"SELECT * FROM experiment WHERE name = '{exp_name}'", engine
    )
    if experiment.empty:
        raise ValueError(f"Experiment {exp_name} not found in the database.")
    experiment_id = experiment.iloc[0]["id"]

    throughput = pd.read_sql_query(
        f"SELECT * FROM throughputs_filtered WHERE experiment_id = '{experiment_id}'",
        engine,
    )
    throughput["real_abw"] = capacity - throughput["throughput"]
    return throughput


def get_interpolated(engine):
    non_interp = (
        pd.read_sql_query("SELECT * FROM non_interpolated_throughputs", engine)
        .set_index("time")
        .sort_index()
    )

    interp_values = (
        non_interp.groupby("subnet", group_keys=False)  # preserve flat index
        .apply(lambda x: x.interpolate(method="time"))
        .reset_index()  # bring “time” back as column
    )

    link_states = pd.read_sql_query("SELECT * FROM link_states_with_subnet", engine)

    # Prepare sender- and receiver-specific interpolated DataFrames
    interp_snd = interp_values.rename(
        columns={"subnet": "subnet_snd", "throughput": "tp_snd", "moving_avg": "ma_snd"}
    )[["time", "subnet_snd", "tp_snd", "ma_snd"]]

    interp_rcv = interp_values.rename(
        columns={"subnet": "subnet_rcv", "throughput": "tp_rcv", "moving_avg": "ma_rcv"}
    )[["time", "subnet_rcv", "tp_rcv", "ma_rcv"]]

    # Merge interpolated values onto link_states
    filled = link_states.merge(interp_snd, on=["time", "subnet_snd"], how="left").merge(
        interp_rcv, on=["time", "subnet_rcv"], how="left"
    )

    # Choose the larger throughput and moving_avg for each row
    filled["throughput"] = filled[["tp_snd", "tp_rcv"]].max(axis=1)
    filled["moving_avg"] = filled[["ma_snd", "ma_rcv"]].max(axis=1)

    filled = filled.drop(columns=["tp_snd", "ma_snd", "tp_rcv", "ma_rcv"])
    return filled


def exp_id_from_name(name):
    return experiment_table[experiment_table["name"] == name]["id"].values[0]


def exp_name_from_id(exp_id):
    return experiment_table[experiment_table["id"] == exp_id]["name"].values[0]


def enrich_interpolated_data(interp_data: pd.DataFrame):
    ret = []
    interp_data = interp_data[interp_data["abw"] != 0]
    interp_data["abw"] = interp_data["abw"] * 8
    interp_data["ip_pair"] = (
        interp_data["sender_ip"] + " -> " + interp_data["receiver_ip"]
    )
    for exp_id, exp_data in interp_data.groupby(("experiment_id")):
        exp_name = exp_name_from_id(exp_id)
        capacity = experiments_db[exp_name]["capacity"]
        exp_data["real_abw"] = capacity - exp_data["moving_avg"]
        ret.append(exp_data)

    return pd.concat(ret)


def add_relative_time(
    df: pd.DataFrame,
    time_col: str = "time",
    new_col: str = "relative_time",
    unit: str = "s",
) -> pd.DataFrame:
    df = df.copy()
    # Ensure the time column is datetime
    df[time_col] = pd.to_datetime(df[time_col])

    # Compute delta from the minimum timestamp
    delta = df[time_col] - df[time_col].min()

    # Compute relative time in the requested unit
    try:
        df[new_col] = delta / pd.Timedelta(1, unit=unit)
    except ValueError:
        raise ValueError(
            f"Unsupported unit '{unit}'. Choose from 's', 'ms', 'm', 'h', 'd'."
        )

    return df


def plot_abw_vs_estimated():
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    # Plot timeseries for each experiment
    for exp_id, exp_data in interp.groupby(("experiment_id")):
        # Calculate moving median for each ip_pair
        experiment_name = exp_name_from_id(exp_id)
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])
        exp_data["abw"] /= 1000000
        exp_data["real_abw"] /= 1000000
        # A) rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"abw": "rolling_abw"})
        )

        # B) rolling for 'real_abw'
        rolled_real = (
            exp_data.set_index("time")
            .groupby("ip_pair")["real_abw"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"real_abw": "rolling_real_abw"})
        )
        exp_data = exp_data.merge(rolled_abw, on=["ip_pair", "time"], how="left").merge(
            rolled_real, on=["ip_pair", "time"], how="left"
        )
        exp_data = add_relative_time(exp_data, unit="m", new_col="reltime")
        # after you’ve prepared exp_data and reltime...
        fig, ax = plt.subplots(figsize=(20, 12))

        # plot your two series without legends
        sns.lineplot(
            data=exp_data,
            x="reltime",
            y="rolling_real_abw",
            hue="ip_pair",
            alpha=0.8,
            legend=False,
            palette="dark:#5A9_r",
            ax=ax,
        )
        sns.lineplot(
            data=exp_data,
            x="reltime",
            y="rolling_abw",
            hue="ip_pair",
            alpha=0.8,
            legend=False,
            palette="ch:s=.25,rot=-.25",
            ax=ax,
        )

        # Extract the first color from each palette for the legend boxes
        pal_real = sns.color_palette("dark:#5A9_r", 1)[0]
        pal_est = sns.color_palette("ch:s=.25,rot=-.25", 1)[0]

        # Create colored box handles for the legend
        handles = [
            Patch(facecolor=pal_est, edgecolor="black", label="Estimated ABW"),
            Patch(facecolor=pal_real, edgecolor="black", label="Real ABW"),
        ]

        # Add the custom legend with colored boxes
        ax.legend(handles=handles, title="Metric", loc="upper left", frameon=True)

        ax.set_title(f"Real vs Estimated ABW (720s moving average) {experiment_name}")
        ax.set_xlabel("Time (minutes)")
        ax.set_ylabel("ABW (Mbit/s)")
        plt.tight_layout()
        fig.savefig(
            os.path.join(out_dir, f"real_abw_vs_estimated{experiment_name}.pdf"),
            format="pdf",
            bbox_inches="tight",
        )
        plt.show()

def plot_exp2_abw_vs_estimated_median():
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    # Plot timeseries for each experiment
    exp2: pd.DataFrame = interp[interp["experiment_id"] == exp_id_from_name("exp2")]
    exp2_fluid: pd.DataFrame = interp[interp["experiment_id"] == exp_id_from_name("exp2_fluid")]

    experiments = [(exp2, "exp2"), (exp2_fluid, "exp2_fluid")]
    data = []
    for exp_data, exp_name in experiments:
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])
        exp_data["abw"] /= 1000000
        exp_data["real_abw"] /= 1000000
        # A) rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw"]
            .rolling("720s")
            .median()
            .reset_index()
            .rename(columns={"abw": "rolling_abw"})
        )

        # B) rolling for 'real_abw'
        rolled_real = (
            exp_data.set_index("time")
            .groupby("ip_pair")["real_abw"]
            .rolling("720s")
            .median()
            .reset_index()
            .rename(columns={"real_abw": "rolling_real_abw"})
        )
        exp_data = exp_data.merge(rolled_abw, on=["ip_pair", "time"], how="left").merge(
            rolled_real, on=["ip_pair", "time"], how="left"
        )
        exp_data = add_relative_time(exp_data, unit="m", new_col="reltime")
        data.append(exp_data)

    exp2_fluid = data[1]
    exp2 = data[0]

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(20, 12), sharey=True)
    sns.lineplot(
        data=exp2,
        x="reltime",
        y="rolling_real_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="dark:#5A9_r",
        ax=ax1,
    )
    sns.lineplot(
        data=exp2,
        x="reltime",
        y="rolling_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="ch:s=.25,rot=-.25",
        ax=ax1,
    )

    sns.lineplot(
        data=exp2_fluid,
        x="reltime",
        y="rolling_real_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="dark:#5A9_r",
        ax=ax2,
    )
    sns.lineplot(
        data=exp2_fluid,
        x="reltime",
        y="rolling_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="ch:s=.25,rot=-.25",
        ax=ax2,
    )
    ax2.axhline(
        y=exp2_fluid["real_abw"].median(), color="red", linestyle="--", label="Median"
    )
    ax2.axhline(
        y=exp2_fluid["abw"].median(), color="blue", linestyle="--", label="Median"
    )
    ax1.axhline(
        y=exp2["real_abw"].median(), color="red", linestyle="--", label="Median"
    )
    ax1.axhline(
        y=exp2["abw"].median(), color="blue", linestyle="--", label="Median"
    )
    # Extract the first color from each palette for the legend boxes
    pal_real = sns.color_palette("dark:#5A9_r", 1)[0]
    pal_est = sns.color_palette("ch:s=.25,rot=-.25", 1)[0]

    # Create colored box handles for the legend
    handles = [
        Patch(facecolor=pal_est, edgecolor="black", label="Estimated ABW"),
        Patch(facecolor=pal_real, edgecolor="black", label="Real ABW"),
        Line2D(
            xdata=[],
            ydata=[],
            color="red",
            linestyle="--",
            label="Median Real ABW",
        ),
        Line2D(
            xdata=[],
            ydata=[],
            color="blue",
            linestyle="--",
            label="Median Estimated ABW",
        ),
    ]

    # Add the custom legend with colored boxes
    ax1.legend(handles=handles, title="Legend", loc="upper left", frameon=True)

    ax1.set_title("Real vs Estimated ABW (720s moving average) exp2")
    ax1.set_xlabel("Time (minutes)")
    ax1.set_ylabel("ABW (Mbit/s)")

    ax2.legend(handles=handles, title="Legend", loc="upper left", frameon=True)
    ax2.set_title("Real vs Estimated ABW (720s moving average) exp2_fluid")
    ax2.set_xlabel("Time (minutes)")
    ax2.set_ylabel("ABW (Mbit/s)")
    plt.tight_layout()
    fig.savefig(
        os.path.join(out_dir, "accuracy_by_real_abw_exp2_exp2_fluid_median.pdf"),
        format="pdf",
        bbox_inches="tight",
    )
    #plt.show()

def plot_exp2_abw_vs_estimated():
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    # Plot timeseries for each experiment
    exp2: pd.DataFrame = interp[interp["experiment_id"] == exp_id_from_name("exp2")]
    exp2_fluid: pd.DataFrame = interp[interp["experiment_id"] == exp_id_from_name("exp2_fluid")]

    experiments = [(exp2, "exp2"), (exp2_fluid, "exp2_fluid")]
    data = []
    for exp_data, exp_name in experiments:
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])
        exp_data["abw"] /= 1000000
        exp_data["real_abw"] /= 1000000
        # A) rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"abw": "rolling_abw"})
        )

        # B) rolling for 'real_abw'
        rolled_real = (
            exp_data.set_index("time")
            .groupby("ip_pair")["real_abw"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"real_abw": "rolling_real_abw"})
        )
        exp_data = exp_data.merge(rolled_abw, on=["ip_pair", "time"], how="left").merge(
            rolled_real, on=["ip_pair", "time"], how="left"
        )
        exp_data = add_relative_time(exp_data, unit="m", new_col="reltime")
        data.append(exp_data)

    exp2_fluid = data[1]
    exp2 = data[0]

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(20, 12), sharey=True)
    sns.lineplot(
        data=exp2,
        x="reltime",
        y="rolling_real_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="dark:#5A9_r",
        ax=ax1,
    )
    sns.lineplot(
        data=exp2,
        x="reltime",
        y="rolling_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="ch:s=.25,rot=-.25",
        ax=ax1,
    )

    sns.lineplot(
        data=exp2_fluid,
        x="reltime",
        y="rolling_real_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="dark:#5A9_r",
        ax=ax2,
    )
    sns.lineplot(
        data=exp2_fluid,
        x="reltime",
        y="rolling_abw",
        hue="ip_pair",
        alpha=0.8,
        legend=False,
        palette="ch:s=.25,rot=-.25",
        ax=ax2,
    )
    ax2.axhline(
        y=exp2_fluid["real_abw"].median(), color="red", linestyle="--", label="Median"
    )
    ax2.axhline(
        y=exp2_fluid["abw"].median(), color="blue", linestyle="--", label="Median"
    )
    ax1.axhline(
        y=exp2["real_abw"].median(), color="red", linestyle="--", label="Median"
    )
    ax1.axhline(
        y=exp2["abw"].median(), color="blue", linestyle="--", label="Median"
    )
    # Extract the first color from each palette for the legend boxes
    pal_real = sns.color_palette("dark:#5A9_r", 1)[0]
    pal_est = sns.color_palette("ch:s=.25,rot=-.25", 1)[0]

    # Create colored box handles for the legend
    handles = [
        Patch(facecolor=pal_est, edgecolor="black", label="Estimated ABW"),
        Patch(facecolor=pal_real, edgecolor="black", label="Real ABW"),
        Line2D(
            xdata=[],
            ydata=[],
            color="red",
            linestyle="--",
            label="Median Real ABW",
        ),
        Line2D(
            xdata=[],
            ydata=[],
            color="blue",
            linestyle="--",
            label="Median Estimated ABW",
        ),
    ]

    # Add the custom legend with colored boxes
    ax1.legend(handles=handles, title="Legend", loc="upper left", frameon=True)

    ax1.set_title("Real vs Estimated ABW (720s moving average) exp2")
    ax1.set_xlabel("Time (minutes)")
    ax1.set_ylabel("ABW (Mbit/s)")

    ax2.legend(handles=handles, title="Legend", loc="upper left", frameon=True)
    ax2.set_title("Real vs Estimated ABW (720s moving average) exp2_fluid")
    ax2.set_xlabel("Time (minutes)")
    ax2.set_ylabel("ABW (Mbit/s)")
    plt.tight_layout()
    fig.savefig(
        os.path.join(out_dir, "accuracy_by_real_abw_exp2_exp2_fluid.pdf"),
        format="pdf",
        bbox_inches="tight",
    )
    plt.show()


def plot_pgm_scatterplot():
    for exp_name, exp_data in experiments_db.items():
        pgm = pgm_filtered_by_timestamp(sql_engine, exp_name, exp_data["max_capacity"])
        pgm["len/gin"] = pgm["len"] / pgm["gin"] / 125000
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]
        used = pgm[pgm["used_in_regression"]]
        unused = pgm[pgm["used_in_regression"] == False]

        # Add legend with percentage of total for used and unused
        used_percentage = len(used) / (len(used) + len(unused)) * 100
        unused_percentage = len(unused) / (len(used) + len(unused)) * 100

        # Create a scatter plot
        fig, ax = plt.subplots(figsize=(18, 10))
        sns.scatterplot(
            data=pgm,
            x="len/gin",
            y="gout/gin",
            hue="used_in_regression",
            palette={True: "#3344ff", False: "#9b9b9b"},
            alpha=0.8,
            s=12,
            ax=ax,
        )
        ax.set(xlabel="len/gin (Mbit/s)", ylabel="gout/gin")
        ax.set_title(f"Probe gap scatterplot {exp_name} (all data)")
        used_patch = Patch(
            facecolor="#2934f6",
            edgecolor="black",
            label=f"Used in regression ({used_percentage:.2f}%)",
        )
        unused_patch = Patch(
            facecolor="#8c8c8c",
            edgecolor="black",
            label=f"Not used in regression ({unused_percentage:.2f}%)",
        )

        ax.legend(handles=[used_patch, unused_patch], title="Legend", loc="upper left")
        plt.xscale("log")
        plt.yscale("log")
        plt.xlim(left=0.1)
        plt.ylim(bottom=0.01)
        plt.show()


def plot_pgm_scatterplot_with_density():
    for exp_name, exp_data in experiments_db.items():
        pgm = pgm_filtered_by_timestamp(sql_engine, exp_name, exp_data["max_capacity"])
        pgm["len/gin"] = pgm["len"] / pgm["gin"] * 8
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]

        pgm = pgm[pgm["len/gin"] < exp_data["max_capacity"]]
        pgm = pgm[pgm["len"] / pgm["gout"] < exp_data["max_capacity"] / 8]

        used = pgm[pgm["used_in_regression"]]
        unused = pgm[pgm["used_in_regression"] == False]

        # Add legend with percentage of total for used and unused
        used_percentage = len(used) / (len(used) + len(unused)) * 100
        unused_percentage = len(unused) / (len(used) + len(unused)) * 100

        # Create a scatter plot
        fig, ax = plt.subplots(figsize=(18, 10))
        sns.scatterplot(
            data=pgm,
            x="len/gin",
            y="gout/gin",
            hue="used_in_regression",
            palette={True: "#3344ff", False: "#9b9b9b"},
            alpha=0.8,
            s=12,
            ax=ax,
        )

        # Print the number of used dps with gout/gin >= 10
        used_high_gout = used[used["gout/gin"] >= 10]
        print(
            f"Number of used dps with gout/gin >= 10: {len(used_high_gout)} {exp_name} total {len(used)}"
        )

        used_patch = Patch(
            facecolor="#2934f6",
            edgecolor="black",
            label=f"Used ({used_percentage:.2f}%)",
        )
        unused_patch = Patch(
            facecolor="#8c8c8c",
            edgecolor="black",
            label=f"Unused ({unused_percentage:.2f}%)",
        )

        ax.legend(handles=[used_patch, unused_patch], loc="upper left")

        ax.set(xlabel="len/gin (bit/s)", ylabel="gout/gin")
        ax.set_title(f"Gap response pattern {exp_name} (Outliers filtered)")
        plt.xscale("log")
        plt.yscale("log")
        plt.xlim(left=exp_data["capacity"]/5, right=exp_data["max_capacity"])
        plt.ylim(bottom=0.1)
        plt.tight_layout()
        # fig.savefig(
        #     os.path.join(out_dir, f"scatterplot_pgm_{exp_name}.png"),
        #     format="png",
        #     bbox_inches="tight",
        # )
        plt.show()


def get_pgm_filtered_counts(engine, exp_name, max_capacity):
    """
    Gets the filtered PGM data from the database.
    """
    experiment = pd.read_sql_query(
        f"SELECT * FROM experiment WHERE name = '{exp_name}'", engine
    )
    if experiment.empty:
        raise ValueError(f"Experiment {exp_name} not found in the database.")
    experiment_id = experiment.iloc[0]["id"]
    query = f"""
SELECT *
FROM get_regression_counts_by_timestamp(
    {max_capacity / 8}::double precision,
    {1362}   ::double precision,
    {experiment_id},
    0.1
);
"""
    ret = pd.read_sql_query(query, engine)
    return ret


def plot_accuracy_per_real_abw_bucket(n_buckets: int = 10):
    """
    Plots the accuracy of the estimator per real ABW bucket.
    n_buckets : int
        Number of equal-frequency buckets used on `real_abw`.
    """
    interp = enrich_interpolated_data(get_interpolated(sql_engine))

    for exp_id, exp_data in interp.groupby("experiment_id"):
        exp_name = exp_name_from_id(exp_id)
        exp_id = exp_id_from_name(exp_name)  # or meta["id"]
        if exp_data.empty:
            print(f"Experiment {exp_name} has no data.")
            continue


        capacity = experiments_db[exp_name]["capacity"]

        # ------------------------------------------------------------------ #
        # 1) build quantile edges and human-readable labels
        quantile_edges = (
            exp_data["real_abw"].quantile(np.linspace(0, 1, n_buckets + 1)).to_numpy()
        )

        # guarantee monotonicity & uniqueness
        quantile_edges[0] = quantile_edges[0] - 1e-9  # include the exact min
        quantile_edges = np.unique(quantile_edges)

        labels = [
            f"{(quantile_edges[i] / 1000000):.2f}–{(quantile_edges[i + 1] / 1000000):.2f}"
            for i in range(len(quantile_edges) - 1)
        ]

        exp_data["real_bucket"] = pd.cut(
            exp_data["real_abw"],
            bins=quantile_edges,
            labels=labels,
            include_lowest=True,
            right=True,
        )

        # ------------------------------------------------------------------ #
        # 2) absolute-percentage error
        exp_data["abs_pct_err"] = (exp_data["abw"] - exp_data["real_abw"]) / capacity * 100

        # get outliers

        # ------------------------------------------------------------------ #
        # 3) plot
        fig, ax = plt.subplots(figsize=(12, 6))
        sns.boxenplot(
            data=exp_data,
            x="real_bucket",
            y="abs_pct_err",
            palette="viridis",
            hue="real_bucket",
            hue_order=labels,
            ax=ax,
            showfliers=True,
        )

        ax.set_xlabel("Real ABW range (Mbit/s) — equal-count ranges")
        ax.set_ylabel("Error (% of bottleneck capacity)")
        ax.set_title(f"Estimator accuracy by real ABW range — {exp_name}")
        ax.set_xticklabels(ax.get_xticklabels(), rotation=45)

        plt.tight_layout()
        fig.savefig(
            os.path.join(out_dir, f"accuracy_by_real_abw_{exp_name}.pdf"),
            format="pdf",
            bbox_inches="tight",
        )
        plt.show()


def plot_pgm_barplot():
    """ """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    for exp_name, exp_data in experiments_db.items():
        counts = get_pgm_filtered_counts(
            sql_engine, exp_name, experiments_db[exp_name]["max_capacity"]
        )
        # add interpolated abw matching on the link_state id
        counts = counts.merge(
            interp[["id", "real_abw", "abw"]],
            left_on="link_state_id",
            right_on="id",
            how="left",
        )

        # use quartile-based bins on used_in_regression
        quantiles = (
            counts["used_in_regression"].quantile([x * 0.1 for x in range(11)]).tolist()
        )
        bins = sorted(set(quantiles))

        # Compute error
        counts = counts.copy()
        counts["error"] = (counts["real_abw"] - counts["abw"]).abs()
        counts_missing = counts[counts["error"].isna()]

        counts_total = counts.copy()

        counts_total["used_bucket"] = pd.cut(
            counts_total["used_in_regression"],
            bins=bins,
            labels=[f"{int(bins[i])}-{int(bins[i + 1])}" for i in range(len(bins) - 1)],
            include_lowest=True,
        )

        percentage_nan = (
            counts_total.groupby("used_bucket")["error"]
            .apply(lambda x: x.isna().sum() / (x.notna().sum() + x.isna().sum()) * 100)
            .reset_index(name="nan_count")
        )

        counts = counts.dropna(subset=["error"])
        counts["error"] = counts["error"] / exp_data["capacity"] * 100

        counts["used_fraq"] = (
            counts["used_in_regression"]
            / (counts["used_in_regression"] + counts["unused_in_regression"])
            * 100
        )

        # ensure unique edges
        labels = [f"{int(bins[i])}-{int(bins[i + 1])}" for i in range(len(bins) - 1)]
        counts_missing["used_bucket"] = pd.cut(
            counts_missing["used_in_regression"],
            bins=bins,
            labels=labels,
            include_lowest=True,
        )
        nan_counts = (
            counts_missing.groupby("used_bucket")["error"]
            .apply(lambda x: x.isna().sum())
            .reset_index(name="nan_count")
        )

        # Create bucket column
        counts["used_bucket"] = pd.cut(
            counts["used_in_regression"], bins=bins, labels=labels, include_lowest=True
        )

        # Aggregate
        summary = counts.groupby("used_bucket")["error"].mean().reset_index()

        # Plot
        f, (ax1, ax2) = plt.subplots(1, 2, figsize=(20, 10))
        sns.barplot(data=summary, x="used_bucket", y="error", palette="flare", ax=ax1)
        sns.barplot(
            data=percentage_nan, x="used_bucket", y="nan_count", palette="flare", ax=ax2
        )
        ax1.set_xlabel("Number of used datapoints")
        ax1.set_ylabel("Error (%)")
        ax1.set_title(f"Mean absolute ABW error by used datapoints ({exp_name})")
        ax2.set_xlabel("Number of used datapoints")
        ax2.set_ylabel("Missing estimates (%)")
        ax2.set_title(f"Missing estimates by used datapoints ({exp_name})")
        ax1.set_xticklabels(ax1.get_xticklabels(), rotation=45)
        ax2.set_xticklabels(ax2.get_xticklabels(), rotation=45)
        plt.tight_layout()
        f.savefig(
            os.path.join(out_dir, f"error_barplot_buckets_{exp_name}.pdf"),
            bbox_inches="tight",
            format="pdf",
        )
        plt.show()


def _x_at_y_equals_one(group: pd.DataFrame) -> float:
    """
    Fit robust linear model  y = b0 + b1*x  (Huber’s T)  for one
    link_state group and return the x where y == 1, i.e.
        x* = (1 - b0) / b1
    Returns NaN if the fit fails or b1 ≈ 0.
    """
    X = sm.add_constant(group["len/gin"].to_numpy())   # [n×2]  (const, x)
    y = group["gout/gin"].to_numpy()

    try:
        res = sm.RLM(y, X).fit()
        b0, b1 = res.params
        if abs(b1) < 1e-9:          # avoid divide-by-zero
            return float("nan")
        return (1.0 - b0) / b1
    except Exception:
        return float("nan")

# ------------------------------------------------------------
# main routine
# ------------------------------------------------------------
def calculate_abw_based_on_pgm_using_robust_regression(n_jobs: int = -1):
    """
    For every experiment:
      · keeps the datapoints marked `used_in_regression`
      · per link_state_id fits a robust line  y=gout/gin vs x=len/gin
      · stores x_at_y1 = (1 - b0)/b1  in column `abw_rlm`
    Returns a dict {exp_name: DataFrame_with_abw_rlm}
    """
    results = {}

    for exp_name, exp_data in experiments_db.items():
        pgm = pgm_filtered_by_timestamp(sql_engine,
                                        exp_name,
                                        exp_data["max_capacity"])

        pgm["len/gin"]  = pgm["len"]  / pgm["gin"]
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]

        data = pgm[pgm["used_in_regression"]].copy()
        link_ids = data["link_state_id"].unique()

        # robust fit in parallel
        x_vals = Parallel(n_jobs=n_jobs, backend="loky")(
            delayed(_x_at_y_equals_one)(data[data["link_state_id"] == lid])
            for lid in link_ids
        )

        abw_rlm = (
            pd.DataFrame({"link_state_id": link_ids, "abw_rlm": x_vals})
              .set_index("link_state_id")
        )

        results[exp_name] = abw_rlm

    return results


def plot_abw_rlm_vs_abw():
    """
    Plots the ABW from the robust regression against the ABW
    from the database.
    """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)

    for exp_id, exp_data in interp.groupby(("experiment_id")):
        # Calculate moving median for each ip_pair
        experiment_name = exp_name_from_id(exp_id)
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])

        abw_rlm = read_data_with_header(f"testcsv_{experiment_name}.csv")
        exp_data = exp_data.merge(
            abw_rlm,
            left_on="id",
            right_on="link_state_id",
            how="left",
        )

        exp_data["abw_rlm"] = exp_data["abw_rlm"] * 8

        # Set all values below 0 or above capacity to NaN
        exp_data["abw_rlm"] = exp_data["abw_rlm"].where(
            (exp_data["abw_rlm"] > 0) & (exp_data["abw_rlm"] < experiments_db[experiment_name]["capacity"]),
            np.nan,
        )
        exp_data["abw_rlm"] /= 1000000
        exp_data["real_abw"] /= 1000000
        # A) rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw_rlm"]
            .rolling("720s")
            .median()
            .reset_index()
            .rename(columns={"abw_rlm": "rolling_abw"})
        )

        # B) rolling for 'real_abw'
        rolled_real = (
            exp_data.set_index("time")
            .groupby("ip_pair")["real_abw"]
            .rolling("720s")
            .median()
            .reset_index()
            .rename(columns={"real_abw": "rolling_real_abw"})
        )
        exp_data = exp_data.merge(rolled_abw, on=["ip_pair", "time"], how="left").merge(
            rolled_real, on=["ip_pair", "time"], how="left"
        )
        exp_data = add_relative_time(exp_data, unit="m", new_col="reltime")
        # after you’ve prepared exp_data and reltime...
        fig, ax = plt.subplots(figsize=(20, 12))

        # plot your two series without legends
        sns.lineplot(
            data=exp_data,
            x="reltime",
            y="rolling_real_abw",
            hue="ip_pair",
            alpha=0.8,
            legend=False,
            palette="dark:#5A9_r",
            ax=ax,
        )
        sns.lineplot(
            data=exp_data,
            x="reltime",
            y="rolling_abw",
            hue="ip_pair",
            alpha=0.8,
            legend=False,
            palette="ch:s=.25,rot=-.25",
            ax=ax,
        )

        # Extract the first color from each palette for the legend boxes
        pal_real = sns.color_palette("dark:#5A9_r", 1)[0]
        pal_est = sns.color_palette("ch:s=.25,rot=-.25", 1)[0]

        # Create colored box handles for the legend
        handles = [
            Patch(facecolor=pal_est, edgecolor="black", label="Estimated ABW"),
            Patch(facecolor=pal_real, edgecolor="black", label="Real ABW"),
        ]

        # Add the custom legend with colored boxes
        ax.legend(handles=handles, title="Metric", loc="upper left", frameon=True)

        ax.set_title(f"Real vs Estimated ABW (720s moving average) {experiment_name}")
        ax.set_xlabel("Time (minutes)")
        ax.set_ylabel("ABW (Mbit/s)")
        plt.tight_layout()
        # fig.savefig(
        #     os.path.join(out_dir, f"real_abw_vs_estimated{experiment_name}.pdf"),
        #     format="pdf",
        #     bbox_inches="tight",
        # )
        plt.show()

def plot_error_boxplot_by_used_in_regression_buckets():
    """
    Draws (1) a boxplot of absolute ABW error per bucket of
    used-in-regression samples and (2) a barplot of the share of
    missing estimates in each bucket.
    """
    interp = enrich_interpolated_data(get_interpolated(sql_engine))

    for exp_name, exp_data in experiments_db.items():
        counts = get_pgm_filtered_counts(
            sql_engine, exp_name, experiments_db[exp_name]["max_capacity"]
        ).merge(
            interp[["id", "real_abw", "abw"]],
            left_on="link_state_id",
            right_on="id",
            how="left",
        )

        # ------------------------------------------------------------------ #
        # bucket preparation (unchanged)
        quantiles = (
            counts["used_in_regression"].quantile([x * 0.1 for x in range(11)]).tolist()
        )
        bins = sorted(set(quantiles))
        labels = [f"{int(bins[i])}-{int(bins[i + 1])}" for i in range(len(bins) - 1)]

        counts["error"] = (counts["real_abw"] - counts["abw"]).abs()
        counts["used_bucket"] = pd.cut(
            counts["used_in_regression"], bins=bins, labels=labels, include_lowest=True
        )

        percentage_nan = (
            counts.groupby("used_bucket")["error"]
            .apply(lambda x: x.isna().mean() * 100)
            .reset_index(name="nan_pct")
        )

        # keep only rows with a finite error for the boxplot
        counts_good = counts.dropna(subset=["error"]).assign(
            error=lambda df: df["error"] / exp_data["capacity"] * 100
        )

        # ------------------------------------------------------------------ #
        # plotting
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(18, 10))

        # BOXPLOT – distribution of absolute error
        sns.boxplot(
            data=counts_good,
            x="used_bucket",
            y="error",
            palette="flare",
            ax=ax1,
            whis=1.5,
            showfliers=True,
        )
        ax1.set_xlabel("Number of used datapoints")
        ax1.set_ylabel("Absolute error (% of capacity)")
        ax1.set_title(f"ABW error distribution by used datapoints ({exp_name})")
        ax1.set_xticklabels(ax1.get_xticklabels(), rotation=45)

        # BARPLOT – intensity of variation (std-dev) per bucket
        #     ─────────────────────────────────────────────────────
        variation = (
            counts_good.groupby("used_bucket")["error"]
            .std()
            .reset_index(name="std_dev")
        )

        sns.barplot(
            data=variation, x="used_bucket", y="std_dev", palette="flare", ax=ax2
        )
        ax2.set_xlabel("Number of used datapoints")
        ax2.set_ylabel("Std. dev. of error (%)")
        ax2.set_title(f"Standard error deviation by used datapoints ({exp_name})")
        ax2.set_xticklabels(ax2.get_xticklabels(), rotation=45)

        plt.tight_layout()
        out_path = os.path.join(out_dir, f"error_boxplot_buckets_{exp_name}.pdf")
        fig.savefig(out_path, bbox_inches="tight", format="pdf")
        plt.show()


def plot_error_boxplot(absolute=False):
    """
    Plots a boxplot of the error.
    """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    if absolute:
        interp["error"] = (interp["abw"] - interp["real_abw"]).abs()
    else:
        interp["error"] = interp["abw"] - interp["real_abw"]
    capacity_map = {
        exp_id_from_name(name): exp_data["capacity"]
        for name, exp_data in experiments_db.items()
    }

    for exp_id, exp_data in interp.groupby("experiment_id"):
        exp_name = exp_name_from_id(exp_id)
        capacity = capacity_map[exp_id]
        exp_data["error"] = exp_data["error"] / capacity * 100
        exp_data["exp_name"] = exp_name
        interp.loc[interp["experiment_id"] == exp_id, "error"] = exp_data["error"]
        interp.loc[interp["experiment_id"] == exp_id, "exp_name"] = exp_name

    medians = interp.groupby("exp_name")["error"].median()

    stats = (
        interp.groupby("exp_name")["error"]
        .quantile([0.25, 0.5, 0.75])
        .unstack()
        .rename(columns={0.25: "q1", 0.5: "median", 0.75: "q3"})
    )
    stats["iqr"] = stats["q3"] - stats["q1"]
    stats["lower_fence"] = stats["q1"] - 1.5 * stats["iqr"]
    stats["upper_fence"] = stats["q3"] + 1.5 * stats["iqr"]

    merged = interp.merge(
        stats[["lower_fence", "upper_fence"]], left_on="exp_name", right_index=True
    )
    outliers = merged[
        (merged["error"] < merged["lower_fence"])
        | (merged["error"] > merged["upper_fence"])
    ]

    palette = sns.color_palette("Set2", n_colors=len(medians))

    fig, ax = plt.subplots(figsize=(20, 10))

    # 1) boxplot without fliers
    sns.boxplot(
        data=interp,
        x="exp_name",
        y="error",
        showfliers=False,
        palette=palette,
        ax=ax,
        hue="exp_name",
        hue_order=medians.index,
        order=medians.index,
    )

    # 2) overlay outliers
    sns.stripplot(
        data=outliers,
        x="exp_name",
        y="error",
        color="gray",
        size=3,
        alpha=0.5,
        jitter=True,
        ax=ax,
    )

    # Build legend handles with median labels
    handles = [
        Patch(
            facecolor=palette[i], edgecolor="black", label=f"{exp}: {medians[exp]:.2f}%"
        )
        for i, exp in enumerate(medians.index)
    ]
    ax.legend(handles=handles, title="Median Error", loc="upper right")

    ax.set_xlabel("Experiment")
    ax.set_ylabel("Error (% of bottleneck capacity)")
    ax.set_title("Error Distribution by experiment")
    plt.xticks(rotation=45)
    plt.tight_layout()
    fig.savefig(
        os.path.join(out_dir, f"error_boxplot_absolute_{absolute}.pdf"),
        format="pdf",
        bbox_inches="tight",
    )
    plt.show()


sql_engine = create_engine(
    "postgresql+psycopg2://postgres:password@localhost:5432/metricsdb"
)
conn_string = "host='localhost' dbname='metricsdb'\
user='postgres' password='password'"
conn = psycopg2.connect(conn_string)
db_cursor = conn.cursor()
experiment_table = pd.read_sql_query("SELECT * FROM experiment", sql_engine)
out_dir = "plots"

if __name__ == "__main__":
    # probe_gap_plots(exp=experiments)
    # timeseries_plots(experiments)
    # boxplots(exp=experiments)

    #
    # plot_pgm_scatterplot()

    os.makedirs(out_dir, exist_ok=True)
    plot_abw_rlm_vs_abw()
    # plot_accuracy_per_real_abw_bucket()
    # plot_error_boxplot()

    # plot_exp2_abw_vs_estimated()
    # plot_abw_vs_estimated()
    # plot_pgm_scatterplot_with_density()

    #plot_error_boxplot_by_used_in_regression_buckets()


    plot_error_boxplot(absolute=True)
    plot_error_boxplot()
    #plot_pgm_barplot()

    # Close the connection
    conn.close()
