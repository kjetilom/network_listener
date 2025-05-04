import colorsys
import os

import matplotlib.patches
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt
import matplotlib
from sqlalchemy import create_engine
import psycopg2
from matplotlib.lines import Line2D
from matplotlib.patches import Patch
import numpy as np
import statsmodels.api as sm
from joblib import Parallel, delayed
import math

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
    },
}


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
        # rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"abw": "rolling_abw"})
        )

        # rolling for 'real_abw'
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
    exp2_fluid: pd.DataFrame = interp[
        interp["experiment_id"] == exp_id_from_name("exp2_fluid")
    ]

    experiments = [(exp2, "exp2"), (exp2_fluid, "exp2_fluid")]
    data = []
    for exp_data, exp_name in experiments:
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])
        exp_data["abw"] /= 1000000
        exp_data["real_abw"] /= 1000000
        # rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw"]
            .rolling("720s")
            .median()
            .reset_index()
            .rename(columns={"abw": "rolling_abw"})
        )

        # rolling for 'real_abw'
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
    ax1.axhline(y=exp2["abw"].median(), color="blue", linestyle="--", label="Median")
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
    plt.show()


def plot_exp2_abw_vs_estimated():
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    # Plot timeseries for each experiment
    exp2: pd.DataFrame = interp[interp["experiment_id"] == exp_id_from_name("exp2")]
    exp2_fluid: pd.DataFrame = interp[
        interp["experiment_id"] == exp_id_from_name("exp2_fluid")
    ]

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
    ax1.axhline(y=exp2["abw"].median(), color="blue", linestyle="--", label="Median")
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
        fig.savefig(
            os.path.join(out_dir, f"probe_gap_scatterplot_{exp_name}_all_data.png"),
            format="png",
            bbox_inches="tight",
        )
        plt.show()


def plot_pgm_scatterplot_without_outliers(with_regression=False, robust=False, savefig=True):
    for exp_name, exp_data in experiments_db.items():
        pgm = pgm_filtered_by_timestamp(sql_engine, exp_name, exp_data["max_capacity"])
        pgm["len/gin"] = pgm["len"] / pgm["gin"] * 8
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]

        pgm = pgm[pgm["len/gin"] < exp_data["max_capacity"]]
        pgm = pgm[pgm["len"] / pgm["gout"] < exp_data["max_capacity"] / 8]

        used = pgm[pgm["used_in_regression"]]
        unused = pgm[pgm["used_in_regression"] == False]

        used_percentage = len(used) / (len(used) + len(unused)) * 100
        unused_percentage = len(unused) / (len(used) + len(unused)) * 100
        intersect_handle = None

        # Create a scatter plot
        fig, ax = plt.subplots(figsize=(18, 10))
        if with_regression:
            if robust:
                # robust fit via statsmodels.RLM
                X = sm.add_constant(used["len/gin"])
                res = sm.RLM(used["gout/gin"], X).fit()
                b0, b1 = res.params
            else:
                # ordinary least-squares
                b1, b0 = np.polyfit(used["len/gin"], used["gout/gin"], 1)

            # x where y == 1   (guard against zero slope)
            x_star = (1.0 - b0) / b1 if abs(b1) > 1e-9 else None
            if x_star and exp_data["capacity"] / 5 < x_star < exp_data["max_capacity"]:
                ax.scatter(x_star, 1.0, color="red", marker="x", s=120, zorder=5)
                intersect_handle = Line2D([0], [0], color='red', marker='x',
                                          linestyle='none',
                                          label=f"Intersect y=1 ({x_star:,.0f} bit/s)")
            # plot the regression line
            x_min = exp_data["capacity"] / 5
            x_max = exp_data["max_capacity"]
            x_line = np.linspace(x_min, x_max, 200)
            y_line = b0 + b1 * x_line

            ax.plot(x_line, y_line,
                color="red",
                linewidth=1.5,
                label="Regression line")

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
        legend_handles = [used_patch, unused_patch]
        if with_regression and intersect_handle is not None:
            legend_handles.append(intersect_handle)

        ax.legend(handles=legend_handles, loc="upper left")

        ax.set(xlabel="len/gin (bit/s)", ylabel="gout/gin")
        ax.set_title(f"Gap response pattern {exp_name} (Outliers filtered)")
        plt.xscale("log")
        plt.yscale("log")
        plt.xlim(left=exp_data["capacity"] / 5, right=exp_data["max_capacity"])
        plt.ylim(bottom=0.1)
        plt.tight_layout()
        if savefig:
            appendix = f"_with_regression{'_robust' if robust else ''}" if with_regression else ""
            fig.savefig(
                os.path.join(out_dir, f"scatterplot_pgm_{exp_name}{appendix}.png"),
                format="png",
                bbox_inches="tight",
            )
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


def plot_accuracy_per_real_abw_bucket(n_buckets: int = 10, rls: bool = False):
    """
    Plots the accuracy of the estimator per real ABW bucket.
    n_buckets : int
        Number of equal-frequency buckets used on `real_abw`.
    """
    interp = enrich_interpolated_data(get_interpolated(sql_engine))
    abw_colname = "abw"
    if rls:
        abw_rlm = read_data_with_header("rlm_results.csv")
        abw_rlm.drop(columns=["experiment_id"], inplace=True)

        interp = interp.merge(
            abw_rlm,
            left_on="id",
            right_on="link_state_id",
            how="left",
        )
        abw_colname = "abw_rls"
        interp["abw_rls"] = interp["abw_rls"] * 8


    for exp_id, exp_data in interp.groupby("experiment_id"):
        exp_name = exp_name_from_id(exp_id)
        exp_id = exp_id_from_name(exp_name)  # or meta["id"]
        if exp_data.empty:
            print(f"Experiment {exp_name} has no data.")
            continue

        capacity = experiments_db[exp_name]["capacity"]

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
        exp_data["abs_pct_err"] = (
            (exp_data[abw_colname] - exp_data["real_abw"]) / capacity * 100
        )

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
            os.path.join(out_dir, f"accuracy_by_real_abw_{exp_name}{"" if not rls else "_rls"}.pdf"),
            format="pdf",
            bbox_inches="tight",
        )
        plt.show()


def plot_pgm_barplot(rls: bool = False):
    """ """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    abw_colname = "abw"
    if rls:
        abw_rlm = read_data_with_header("rlm_results.csv")
        abw_rlm.drop(columns=["experiment_id"], inplace=True)

        interp = interp.merge(
            abw_rlm,
            left_on="id",
            right_on="link_state_id",
            how="left",
        )
        abw_colname = "abw_rls"
        interp["abw_rls"] = interp["abw_rls"] * 8

    for exp_name, exp_data in experiments_db.items():
        counts = get_pgm_filtered_counts(
            sql_engine, exp_name, experiments_db[exp_name]["max_capacity"]
        )
        # add interpolated abw matching on the link_state id
        counts = counts.merge(
            interp[["id", "real_abw", abw_colname]],
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
        counts["error"] = (counts["real_abw"] - counts[abw_colname]).abs()
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
            os.path.join(out_dir, f"error_barplot_buckets_{exp_name}{"" if not rls else "_rls"}.pdf"),
            bbox_inches="tight",
            format="pdf",
        )
        plt.show()


def _fit_rls(group: pd.DataFrame, phy_cap_Bps: float) -> float | None:
    X = sm.add_constant(group["len/gin"].values)
    y = group["gout/gin"].values
    try:
        res = sm.RLM(y, X).fit()
        b0, b1 = res.params
        if b1 <= 0:
            return None
    except Exception:
        return None
    if abs(b1) < 1e-9:
        return None
    abw = (1.0 - b0) / b1                    # bytes * s⁻¹
    return abw if 0.0 < abw < phy_cap_Bps / 8.0 else None


def calculate_abw_based_on_pgm_using_robust_regression(
    n_jobs: int = -1
) -> pd.DataFrame:
    """
    Robust ABW estimate for every (experiment, link_state_id) pair.
    Returns
    -------
    DataFrame with columns
        ['link_state_id', 'experiment_id', 'abw_rls']
    """
    rows = []

    for exp_name, meta in experiments_db.items():
        pgm = pgm_filtered_by_timestamp(sql_engine,
                                        exp_name,
                                        meta["max_capacity"])

        pgm["len/gin"]  = pgm["len"]  / pgm["gin"]
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]

        data = pgm[pgm["used_in_regression"]]
        if data.empty:
            continue

        exp_id    = exp_id_from_name(exp_name)
        phy_cap_Bps = meta["max_capacity"]          # already Bytes/s
        link_ids  = data["link_state_id"].unique()

        abw_vals = Parallel(n_jobs=n_jobs, backend="loky")(
            delayed(_fit_rls)(
                data[data["link_state_id"] == lid],
                phy_cap_Bps
            )
            for lid in link_ids
        )

        rows.extend(
            {"link_state_id": lid,
             "experiment_id": exp_id,
             "abw_rls": abw}
            for lid, abw in zip(link_ids, abw_vals)
        )

    return pd.DataFrame(rows)


def plot_experiments_pair_grid(exp_names: list[str],
                               k_per_exp: int = 4,
                               pts_min: int = 40):
    """
    For each experiment in *exp_names*:
      • pick the k_per_exp link_state_ids with most samples
      • plot them two per row in their own figure
      • regression line + y=1 intersect on every subplot
    """

    def darker(rgb, factor=0.55):
        import colorsys
        h, l, s = colorsys.rgb_to_hls(*rgb)
        return colorsys.hls_to_rgb(h, max(0, l * factor), s)

    for exp in exp_names:
        meta = experiments_db[exp]
        pgm  = pgm_filtered_by_timestamp(sql_engine, exp, meta["max_capacity"])

        pgm["len/gin"]  = pgm["len"]  / pgm["gin"] * 8
        pgm["gout/gin"] = pgm["gout"] / pgm["gin"]

        pgm = pgm[(pgm["len/gin"] < meta["max_capacity"])
                  & (pgm["len"] / pgm["gout"] < meta["max_capacity"] / 8)]

        used = pgm[pgm["used_in_regression"]]

        # pick the k largest link_state_id groups
        link_ids = (used.groupby("link_state_id")
                         .size()
                         .sort_values(ascending=False)
                         .head(k_per_exp)
                         .index)

        if link_ids.empty:
            print(f"[WARN] {exp}: no link_state_id with ≥{pts_min} pts")
            continue

        n_rows = math.ceil(len(link_ids) / 2)
        fig, axes = plt.subplots(n_rows, 2,
                                 figsize=(12, 5 * n_rows),
                                 squeeze=False)

        base_clr = sns.color_palette("Set2", len(link_ids))

        for i, lid in enumerate(link_ids):
            d = used[used["link_state_id"] == lid]
            r, c = divmod(i, 2)
            ax = axes[r][c]

            # skip sparse groups
            if d.shape[0] < pts_min:
                ax.axis("off")
                continue

            slope, intercept = np.polyfit(d["len/gin"], d["gout/gin"], 1)
            if slope <= 0:
                ax.axis("off")
                continue

            x_star = (1.0 - intercept) / slope
            x_span = np.linspace(min(d["len/gin"].min(), x_star * .8),
                                 max(x_star, d["len/gin"].max()), 200)
            y_span = intercept + slope * x_span

            sns.scatterplot(data=d, x="len/gin", y="gout/gin",
                            s=18, color=base_clr[i], ax=ax, alpha=0.8)
            ax.plot(x_span, y_span, color=darker(base_clr[i]), lw=1.2)
            ax.axhline(1, color='grey', ls='--', lw=0.7)
            ax.scatter(x_star, 1, color='red', marker='x', s=60, zorder=4)

            ax.set_title(f"{exp} – link {lid}", fontsize=10)
            ax.set_xscale('log'); ax.set_yscale('log')
            ax.set_xlabel("len/gin (Mbit/s)")
            ax.set_ylabel("gout/gin")
            ax.set_ylim(0.1, max(1.1, d["gout/gin"].max()))
            ax.set_xlim(meta["capacity"]/5, meta["max_capacity"]*1.1)

        # hide any unused subplot axes
        for j in range(len(link_ids), n_rows*2):
            r, c = divmod(j, 2)
            axes[r][c].axis("off")

        fig.suptitle(f"Probe‑gap scatter (top {k_per_exp} links) – {exp}",
                     fontsize=14)
        plt.tight_layout()
        plt.show()



def plot_abw_rlm_vs_abw():
    """
    Plots the ABW from the robust regression against the ABW
    from the database.
    """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    abw_rlm = read_data_with_header("rlm_results.csv")
    abw_rlm.drop(columns=["experiment_id"], inplace=True)

    interp = interp.merge(
        abw_rlm,
        left_on="id",
        right_on="link_state_id",
        how="left",
    )
    interp["abw_rls"] = interp["abw_rls"] * 8

    for exp_id, exp_data in interp.groupby(("experiment_id")):
        # Calculate moving median for each ip_pair
        experiment_name = exp_name_from_id(exp_id)
        exp_data["time"] = pd.to_datetime(exp_data["time"])
        exp_data = exp_data.sort_values(["ip_pair", "time"])

        # Set all values below 0 or above capacity to NaN
        exp_data["abw_rls"] = exp_data["abw_rls"].where(
            (exp_data["abw_rls"] > 0)
            & (exp_data["abw_rls"] < experiments_db[experiment_name]["capacity"]),
            np.nan,
        )
        exp_data["abw_rls"] /= 1000000
        exp_data["real_abw"] /= 1000000
        #  rolling for 'abw'
        rolled_abw = (
            exp_data.set_index("time")
            .groupby("ip_pair")["abw_rls"]
            .rolling("720s")
            .mean()
            .reset_index()
            .rename(columns={"abw_rls": "rolling_abw"})
        )

        #  rolling for 'real_abw'
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
        fig, ax = plt.subplots(figsize=(20, 12))

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
            Patch(facecolor=pal_est, edgecolor="black", label="Estimated ABW (Robust regression)"),
            Patch(facecolor=pal_real, edgecolor="black", label="Real ABW"),
        ]

        # Add the custom legend with colored boxes
        ax.legend(handles=handles, title="Metric", loc="upper left", frameon=True)

        ax.set_title(f"Real vs Estimated ABW (720s moving average) {experiment_name} (Robust regression)")
        ax.set_xlabel("Time (minutes)")
        ax.set_ylabel("ABW (Mbit/s)")
        plt.tight_layout()
        fig.savefig(
            os.path.join(out_dir, f"real_abw_vs_estimated_{experiment_name}_robust.pdf"),
            format="pdf",
            bbox_inches="tight",
        )
        plt.show()


def plot_error_boxplot_by_used_in_regression_buckets(rls=False):
    """
    Draws (1) a boxplot of absolute ABW error per bucket of
    used-in-regression samples and (2) a barplot of the share of
    missing estimates in each bucket.
    """
    interp = enrich_interpolated_data(get_interpolated(sql_engine))
    abw_colname = "abw"
    if rls:
        abw_rlm = read_data_with_header("rlm_results.csv")
        abw_rlm.drop(columns=["experiment_id"], inplace=True)

        interp = interp.merge(
            abw_rlm,
            left_on="id",
            right_on="link_state_id",
            how="left",
        )
        abw_colname = "abw_rls"
        interp["abw_rls"] = interp["abw_rls"] * 8

    for exp_name, exp_data in experiments_db.items():
        counts = get_pgm_filtered_counts(
            sql_engine, exp_name, experiments_db[exp_name]["max_capacity"]
        ).merge(
            interp[["id", "real_abw", abw_colname]],
            left_on="link_state_id",
            right_on="id",
            how="left",
        )

        quantiles = (
            counts["used_in_regression"].quantile([x * 0.1 for x in range(11)]).tolist()
        )
        bins = sorted(set(quantiles))
        labels = [f"{int(bins[i])}-{int(bins[i + 1])}" for i in range(len(bins) - 1)]

        counts["error"] = (counts["real_abw"] - counts[abw_colname]).abs()
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
        out_path = os.path.join(out_dir, f"error_boxplot_buckets_{exp_name}{"" if not rls else "_rls"}.pdf")
        fig.savefig(out_path, bbox_inches="tight", format="pdf")
        plt.show()



def plot_error_boxplot_dual():
    """
    Plots a boxplot of the error.
    """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    abw_rlm = read_data_with_header("rlm_results.csv")
    abw_rlm.drop(columns=["experiment_id"], inplace=True)

    interp = interp.merge(
        abw_rlm,
        left_on="id",
        right_on="link_state_id",
        how="left",
    )
    interp["abw_rls"] = interp["abw_rls"] * 8
    interp["error"] = interp["abw"] - interp["real_abw"]
    interp["error_rls"] = interp["abw_rls"] - interp["real_abw"]

    capacity_map = {
        exp_id_from_name(name): exp_data["capacity"]
        for name, exp_data in experiments_db.items()
    }

    for exp_id, exp_data in interp.groupby("experiment_id"):
        exp_name = exp_name_from_id(exp_id)
        capacity = capacity_map[exp_id]
        exp_data["error"] = exp_data["error"] / capacity * 100
        exp_data["error_rls"] = exp_data["error_rls"] / capacity * 100
        exp_data["exp_name"] = exp_name
        interp.loc[interp["experiment_id"] == exp_id, "error"] = exp_data["error"]
        interp.loc[interp["experiment_id"] == exp_id, "error_rls"] = exp_data["error_rls"]
        interp.loc[interp["experiment_id"] == exp_id, "exp_name"] = exp_name

    # collapse the long DataFrame into “tidy” form for seaborn
    long = (
        interp.melt(
            id_vars=["exp_name"],
            value_vars=["error", "error_rls"],
            var_name="estimator",
            value_name="err_pct",
        )
        .dropna(subset=["err_pct"])          # keep only finite errors
    )

    # helper to darken a colour
    def darker(rgb, factor=0.55):
        h, l, s = colorsys.rgb_to_hls(*rgb)
        return colorsys.hls_to_rgb(h, max(0, l * factor), s)

    # ordered experiments for x-axis
    exp_order = sorted(long["exp_name"].unique())

    fig, ax = plt.subplots(figsize=(20, 10))

    # draw with *only two* hue levels = ['error', 'error_rls']
    sns.boxplot(
        data=long,
        x="exp_name",
        y="err_pct",
        hue="estimator",          # just 2 categories
        order=exp_order,
        dodge=True,
        width=0.8,               # narrower so the pair nearly touch
        whis=1.5,
        fliersize=2,
        ax=ax,
        palette=["#cccccc", "#888888"]  # throw-away palette, will be recoloured
    )

    # recolour every pair: plain = Set2, robust = darker(Set2)
    base_set2 = sns.color_palette("Set2", len(exp_order))
    base_set2.extend([darker(c) for c in base_set2])
    boxes = ax.findobj(matplotlib.patches.PathPatch)
    for color, box in zip(base_set2, boxes):
        box.set_facecolor(color)
        box.set_edgecolor("black")
        box.set_alpha(0.90)

    overall_meds = (
        long.groupby(["exp_name", "estimator"])["err_pct"].median()
    )

    # Add an average of all experiments with exp name "overall"
    rls_mean = overall_meds.unstack()["error_rls"].mean()
    plain_mean = overall_meds.unstack()["error"].mean()
    print(rls_mean, plain_mean)
    df_to_latex_table(
        overall_meds.unstack().round(2),
        file=os.path.join(out_dir, "error_boxplot_dual.tex"),
    )
    # handles = []
    # for color, (exp_name, estimator) in zip(base_set2, overall_meds.index):
    #     if estimator == "error":
    #         label = f"{exp_name} {overall_meds[exp_name, 'error']:.2f}%"
    #     else:
    #         label = f"{exp_name}-rls {overall_meds[exp_name, 'error']:.2f}%"
    #     handles.append(
    #         Patch(facecolor=color, edgecolor="black", label=label)
    #     )

    overall_meds = (
    long.groupby("estimator")["err_pct"]
         .median()
         .rename({"error": "Plain", "error_rls": "Robust-LS"})
)

    handles = [
        Patch(facecolor=base_set2[0],           edgecolor='black',
            label=f"Simple linear regression (median {overall_meds['Plain']:.2f} %)"),
        Patch(facecolor=darker(base_set2[0]),   edgecolor='black',
            label=f"Robust least squares (median {overall_meds['Robust-LS']:.2f} %)")
    ]
    ax.legend(handles=handles, title="Estimator", loc="upper right", ncol=1)

    # ax.legend(handles=handles, title="Estimator", loc="upper right", ncol=2)

    ax.set_xlabel("Experiment")
    ax.set_ylabel("Error (% of bottleneck capacity)")
    ax.set_title("Accuracy of ABW estimator by experiment and regression method")
    ax.set_xticklabels(ax.get_xticklabels(), rotation=45)
    plt.tight_layout()
    fig.savefig(
        os.path.join(out_dir, f"error_boxplot_dual.pdf"),
        format="pdf",
        bbox_inches="tight",
    )
    plt.show()


def df_to_latex_table(
    df: pd.DataFrame,
    file: str | None = None,
    col_format: str | None = None,
    float_fmt: str = "%.2f"
) -> str:
    """
    Convert *df* to a LaTeX table wrapped in a floating environment.

    Parameters
    ----------
    df        : DataFrame to convert.
    file      : optional path; if given, write the LaTeX to this file.
    col_format: optional LaTeX column spec (e.g. 'lccc').  If None,
                pandas chooses one automatically.
    float_fmt : printf-style format for floats.

    Returns
    -------
    str  - the LaTeX code.
    """
    latex_body = df.to_latex(
        index=True,
        escape=False,          # allow math/LaTeX in cells
        column_format=col_format,
        float_format=float_fmt.__mod__        # hack: pass formatter
    )

    wrapper = (
        "\\begin{table}[htbp]\n"
        "  \\centering\n"
        f"{latex_body.rstrip()}\n"
        "  \\caption{}\n"
        "  \\label{}\n"
        "\\end{table}\n"
    )

    if file:
        with open(file, "w", encoding="utf-8") as fh:
            fh.write(wrapper)

    return wrapper



def plot_error_boxplot(absolute=False, rls=False):
    """
    Plots a boxplot of the error.
    """
    interp = get_interpolated(sql_engine)
    interp = enrich_interpolated_data(interp)
    abw_colname = "abw"
    if rls:
        abw_rlm = read_data_with_header("rlm_results.csv")
        abw_rlm.drop(columns=["experiment_id"], inplace=True)

        interp = interp.merge(
            abw_rlm,
            left_on="id",
            right_on="link_state_id",
            how="left",
        )
        abw_colname = "abw_rls"
        interp["abw_rls"] = interp["abw_rls"] * 8

    if absolute:
        interp["error"] = (interp[abw_colname] - interp["real_abw"]).abs()
    else:
        interp["error"] = interp[abw_colname] - interp["real_abw"]
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
        os.path.join(out_dir, f"error_boxplot_absolute_{absolute}{"" if not rls else "_rls"}.pdf"),
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
    plot_experiments_pair_grid(
        ["exp2", "exp2_fluid"],
        k_per_exp=6,    # show 3 rows (6 groups) each experiment
        pts_min=30
    )
    plot_pgm_scatterplot_without_outliers(with_regression=True, savefig=True)
    plot_pgm_scatterplot_without_outliers()
    plot_error_boxplot_dual()
    # results = calculate_abw_based_on_pgm_using_robust_regression(10)
    # results.to_csv("rlm_results.csv", index=False)
    # plot_accuracy_per_real_abw_bucket()
    # plot_accuracy_per_real_abw_bucket(rls=True)
    # plot_error_boxplot(absolute=True, rls=True)
    # plot_error_boxplot(rls=True)

    # plot_pgm_barplot(rls=True)

    # plot_error_boxplot_by_used_in_regression_buckets(rls=True)
    # plot_error_boxplot_by_used_in_regression_buckets()
    # plot_abw_rlm_vs_abw()
    # plot_pgm_scatterplot()
    #



    # plot_exp2_abw_vs_estimated()
    # plot_abw_vs_estimated()


    plot_pgm_barplot()

    # Close the connection
    conn.close()
