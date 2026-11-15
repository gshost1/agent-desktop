"""Small exact statistics for the held-out report: Wilson intervals, Fisher's exact test, the sign test."""
import math

Z95 = 1.959964


def wilson(k, n, z=Z95):
    """(low, high) Wilson score interval for k successes in n trials; (0, 1) for n = 0."""
    if n == 0:
        return 0.0, 1.0
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return max(0.0, centre - half), min(1.0, centre + half)


def _table_p(a, b, c, d):
    n = a + b + c + d
    return math.comb(a + b, a) * math.comb(c + d, c) / math.comb(n, a + c)


def fisher(k1, n1, k2, n2):
    """Two-sided Fisher exact p for k1/n1 against k2/n2: the summed probability of every table with the same
    margins that is no more likely than the observed one."""
    col = k1 + k2
    observed = _table_p(k1, n1 - k1, k2, n2 - k2)
    total = 0.0
    for a in range(max(0, col - n2), min(n1, col) + 1):
        p = _table_p(a, n1 - a, col - a, n2 - col + a)
        if p <= observed * (1 + 1e-9):
            total += p
    return min(1.0, total)


def sign_test(wins, losses):
    """Two-sided exact sign test p over the untied pairs."""
    n = wins + losses
    if n == 0:
        return 1.0
    tail = sum(math.comb(n, i) for i in range(0, min(wins, losses) + 1)) / 2 ** n
    return min(1.0, 2 * tail)
