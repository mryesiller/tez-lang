type Light = "red" | "green" | "yellow";

function next(l: Light): Light {
  switch (l) {
    case "red":
      return "green";
    case "green":
      return "yellow";
    case "yellow":
      return "red";
  }
}

function seconds(l: Light): number {
  switch (l) {
    case "red":
      return 30;
    case "green":
      return 25;
    case "yellow":
      return 5;
  }
}

function cycle(l: Light): number {
  return seconds(l) + seconds(next(l)) + seconds(next(next(l)));
}

function main(): [Light, number] {
  return [next("yellow"), cycle("green")];
}
