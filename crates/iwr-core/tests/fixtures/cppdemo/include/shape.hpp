/**
 * Shapes: the things the canvas draws.
 */

#ifndef SHAPE_HPP
#define SHAPE_HPP

#include <string>
#include <vector>

namespace geo {

/// How a shape is filled.
enum class Fill { None, Solid };

/// Anything with an area.
class Shape {
public:
    virtual ~Shape() = default;
    /// The surface, in square units.
    virtual double area() const = 0;
    /// A one-line description.
    std::string describe() const;

protected:
    Fill fill = Fill::None;
};

/// A circle, by its radius.
class Circle : public Shape {
public:
    explicit Circle(double r) : radius(r) {}
    double area() const override;
    /// Grow by a factor.
    void scale(double k) { radius *= k; }

private:
    double radius;
};

/// A rectangle.
struct Rect : Shape {
    double w;
    double h;
    Rect(double w, double h);
    double area() const override { return w * h; }
};

using Shapes = std::vector<Shape *>;

/// The sum of the areas.
double total_area(const Shapes &shapes);

} // namespace geo

#endif
