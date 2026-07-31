function calculate(birthYear: number): number[] {
  const approxAge = getCurrentYear() - birthYear;
  const yearsSince18 = (getCurrentYear() - birthYear) - 18;

  return [approxAge, yearsSince18];
}
