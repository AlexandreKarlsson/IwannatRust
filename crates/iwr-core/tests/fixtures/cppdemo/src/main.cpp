// Draws a few shapes and prints their total area.

#include "shape.hpp"
#include <iostream>

/// Build the shapes of the demo.
static geo::Shapes make_shapes() {
    geo::Shapes out;
    out.push_back(new geo::Circle(2.0));
    out.push_back(new geo::Rect(3.0, 4.0));
    return out;
}

int main(int argc, char **argv) {
    geo::Shapes shapes = make_shapes();
    try {
        double total = geo::total_area(shapes);
        std::cout << "total " << total << std::endl;
    } catch (const std::exception &e) {
        std::cerr << e.what() << std::endl;
        return 1;
    }
    for (auto *s : shapes) {
        switch (argc) {
        case 1:
            std::cout << s->describe() << std::endl;
            break;
        default:
            break;
        }
        delete s;
    }
    return 0;
}
