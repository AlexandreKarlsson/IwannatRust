// Areas and descriptions of the shapes.

#include "shape.hpp"
#include <stdexcept>

namespace geo {

static const double PI = 3.14159;

std::string Shape::describe() const {
    // the area decides the word
    if (area() > 100.0) {
        return "big";
    }
    return "small";
}

double Circle::area() const {
    return PI * radius * radius;
}

Rect::Rect(double w, double h) : w(w), h(h) {
    if (w < 0 || h < 0) {
        throw std::invalid_argument("negative side");
    }
}

double total_area(const Shapes &shapes) {
    double sum = 0;
    for (const Shape *s : shapes) {
        sum += s->area();
    }
    return sum;
}

template <typename T>
T clamp(T v, T lo, T hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

} // namespace geo
