// The held-out web tasks of bench/tezbench/web.mjs. Their ideas must not be trained on (see data/v2/apps.mjs).
export const webTasks = [
  { id: "w1-counter", prompt: "A web page with a counter: a button that adds one, a button that resets to zero, and the count shown. The count must never be negative." },
  { id: "w2-toggle", prompt: "A web page with a button that shows or hides a paragraph of text each time it is clicked." },
  { id: "w3-converter", prompt: "A web page with a text box where the visitor types a temperature in Celsius; the page shows it in Fahrenheit (whole numbers are fine)." },
  { id: "w4-stopwatch", prompt: "A web page that shows how many seconds have passed since it opened, updating every second, with a reset button." },
  { id: "w5-greeting", prompt: "A web page that greets the visitor with 'Good morning' before noon and 'Good afternoon' after noon, from the wall clock, and refreshes every minute." },
  { id: "w6-quote", prompt: "A web page with a button that loads the text file quote.txt from this site and shows it, or shows an error message if loading fails." },
  { id: "w7-todo", prompt: "A to-do list page: a text box and an Add button that appends the typed item to the list shown on the page; each item has a Remove button." },
  { id: "w8-visits", prompt: "A visit counter page that remembers its number in the browser between page loads and shows it." },
];
