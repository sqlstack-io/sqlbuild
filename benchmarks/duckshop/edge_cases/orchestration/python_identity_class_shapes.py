"""Classes that `inspect` locates by parsing: shadowed, nested, local, decorated and trailing."""

import functools

if True:

    class Shadowed:
        def first(self):
            return 1


class Shadowed:  # noqa: F811
    def second(self):
        return 2


class Outer:
    class Inner:
        class Deepest:
            pass

    def method(self):
        class Local:
            pass

        return Local


@functools.total_ordering
class Decorated:
    def __lt__(self, other):
        return False

    def __eq__(self, other):
        return True


class Trailing:
    value = 1
    # trailing comment


def factory():
    class Made:
        pass

    return Made


LocalMade = factory()
OuterLocal = Outer().method()
